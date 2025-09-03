use crate::infra::api::firecrawl::{FirecrawlClient, ReqwestFirecrawlClient};
use crate::infra::storage::db::DatabaseInsertResult;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

// Firecrawl記事構造体（テーブル定義と一致）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Article {
    pub url: String,
    pub timestamp: DateTime<Utc>,
    pub status_code: i32,
    pub content: String,
}

// 記事の処理状態を表現するenum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArticleStatus {
    /// 記事が未処理（articleテーブルに存在しない）
    Unprocessed,
    /// 記事が正常に取得済み（status_code = 200）
    Success,
    /// 記事の取得にエラーが発生（status_code != 200）
    Error(i32),
}

// RSSリンクと記事の紐付き状態を表現する構造体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RssLinkWithArticle {
    pub link: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub article_url: Option<String>,
    pub article_timestamp: Option<DateTime<Utc>>,
    pub article_status_code: Option<i32>,
    pub article_content: Option<String>,
}

impl RssLinkWithArticle {
    /// 記事の処理状態を取得
    pub fn get_article_status(&self) -> ArticleStatus {
        match self.article_status_code {
            None => ArticleStatus::Unprocessed,
            Some(200) => ArticleStatus::Success,
            Some(code) => ArticleStatus::Error(code),
        }
    }

    /// 未処理のリンクかどうかを判定
    pub fn is_unprocessed(&self) -> bool {
        matches!(self.get_article_status(), ArticleStatus::Unprocessed)
    }

    /// エラー状態のリンクかどうかを判定
    pub fn is_error(&self) -> bool {
        matches!(self.get_article_status(), ArticleStatus::Error(_))
    }

    /// 処理が必要なリンクかどうかを判定（未処理またはエラー）
    pub fn needs_processing(&self) -> bool {
        self.is_unprocessed() || self.is_error()
    }
}

// RSSリンクと記事のJOINクエリ用の条件構造体
#[derive(Debug, Default)]
pub struct RssLinkArticleQuery {
    pub link_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
    pub article_status: Option<ArticleStatus>,
    pub limit: Option<i64>,
}

/// 記事をデータベースに保存する。
/// 重複した場合には更新を行う。
pub async fn store_article(article: &Article, pool: &PgPool) -> Result<DatabaseInsertResult> {
    let mut tx = pool
        .begin()
        .await
        .context("トランザクションの開始に失敗しました")?;

    let result = sqlx::query!(
        r#"
        INSERT INTO articles (url, status_code, content)
        VALUES ($1, $2, $3)
        ON CONFLICT (url) DO UPDATE SET 
            status_code = EXCLUDED.status_code,
            content = EXCLUDED.content,
            timestamp = CURRENT_TIMESTAMP
        "#,
        article.url,
        article.status_code,
        article.content
    )
    .execute(&mut *tx)
    .await
    .context("Firecrawl記事のデータベースへの挿入に失敗しました")?;

    let inserted = if result.rows_affected() > 0 { 1 } else { 0 };

    tx.commit()
        .await
        .context("トランザクションのコミットに失敗しました")?;

    Ok(DatabaseInsertResult::new(inserted, 1 - inserted))
}

// Article記事のフィルター条件を表す構造体
#[derive(Debug, Default)]
pub struct ArticleQuery {
    pub url_pattern: Option<String>,
    pub timestamp_from: Option<DateTime<Utc>>,
    pub timestamp_to: Option<DateTime<Utc>>,
    pub status_code: Option<i32>,
}

/// 指定されたデータベースプールからArticleを取得する。
pub async fn search_articles(query: Option<ArticleQuery>, pool: &PgPool) -> Result<Vec<Article>> {
    let query = query.unwrap_or_default();

    // QueryBuilderベースで動的にクエリを構築
    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT url, timestamp, status_code, content FROM articles",
    );

    let has_cond = query.url_pattern.is_some()
        || query.timestamp_from.is_some()
        || query.timestamp_to.is_some()
        || query.status_code.is_some();

    if has_cond {
        qb.push(" WHERE ");
        let mut separated = qb.separated(" AND ");

        if let Some(ref url_pattern) = query.url_pattern {
            let url_query = format!("%{}%", url_pattern);
            separated.push("url ILIKE ").push_bind(url_query);
        }
        if let Some(ts_from) = query.timestamp_from {
            separated.push("timestamp >= ").push_bind(ts_from);
        }
        if let Some(ts_to) = query.timestamp_to {
            separated.push("timestamp <= ").push_bind(ts_to);
        }
        if let Some(status) = query.status_code {
            separated.push("status_code = ").push_bind(status);
        }
    }

    qb.push(" ORDER BY timestamp DESC");

    let articles = qb.build_query_as::<Article>().fetch_all(pool).await?;

    Ok(articles)
}

/// RSSリンクと記事の紐付き状態を取得する
pub async fn search_rss_links_with_articles(
    query: Option<RssLinkArticleQuery>,
    pool: &PgPool,
) -> Result<Vec<RssLinkWithArticle>> {
    let query = query.unwrap_or_default();

    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        r#"
        SELECT 
            rl.link,
            rl.title,
            rl.pub_date,
            a.url as article_url,
            a.timestamp as article_timestamp,
            a.status_code as article_status_code,
            a.content as article_content
        FROM rss_links rl
        LEFT JOIN articles a ON rl.link = a.url
        "#,
    );

    let mut has_where = false;

    // link_pattern query
    if let Some(ref link_pattern) = query.link_pattern {
        if !has_where {
            qb.push(" WHERE ");
            has_where = true;
        }
        let pattern = format!("%{}%", link_pattern);
        qb.push("rl.link ILIKE ").push_bind(pattern);
    }

    // pub_date_from query
    if let Some(pub_date_from) = query.pub_date_from {
        if has_where {
            qb.push(" AND ");
        } else {
            qb.push(" WHERE ");
            has_where = true;
        }
        qb.push("rl.pub_date >= ").push_bind(pub_date_from);
    }

    // pub_date_to query
    if let Some(pub_date_to) = query.pub_date_to {
        if has_where {
            qb.push(" AND ");
        } else {
            qb.push(" WHERE ");
            has_where = true;
        }
        qb.push("rl.pub_date <= ").push_bind(pub_date_to);
    }

    // article_status query
    if let Some(ref status) = query.article_status {
        if has_where {
            qb.push(" AND ");
        } else {
            qb.push(" WHERE ");
        }

        match status {
            ArticleStatus::Unprocessed => {
                qb.push("a.url IS NULL");
            }
            ArticleStatus::Success => {
                qb.push("a.status_code = 200");
            }
            ArticleStatus::Error(code) => {
                qb.push("a.status_code = ").push_bind(*code);
            }
        }
    }

    qb.push(" ORDER BY rl.pub_date DESC");

    // limit
    if let Some(limit) = query.limit {
        qb.push(" LIMIT ").push_bind(limit);
    }

    let results = qb
        .build_query_as::<RssLinkWithArticle>()
        .fetch_all(pool)
        .await
        .context("RSSリンクと記事の紐付き状態取得に失敗")?;

    Ok(results)
}

/// 未処理のRSSリンクを取得する（articleテーブルに存在しないか、status_code != 200）
pub async fn search_unprocessed_rss_links(
    pool: &PgPool,
) -> Result<Vec<crate::domain::rss::RssLink>> {
    let links = sqlx::query_as!(
        crate::domain::rss::RssLink,
        r#"
        SELECT rl.link, rl.title, rl.pub_date
        FROM rss_links rl
        LEFT JOIN articles a ON rl.link = a.url
        WHERE a.url IS NULL OR a.status_code != 200
        ORDER BY rl.pub_date DESC
        LIMIT 100
        "#
    )
    .fetch_all(pool)
    .await
    .context("未処理RSSリンクの取得に失敗")?;

    Ok(links)
}

/// 処理が必要なRSSリンクをRssLinkWithArticle形式で取得する
pub async fn search_rss_links_needing_processing(
    pool: &PgPool,
    limit: Option<i64>,
) -> Result<Vec<RssLinkWithArticle>> {
    let query = RssLinkArticleQuery {
        article_status: None, // 全ての状態を取得してフィルター
        limit,
        ..Default::default()
    };

    let all_links = search_rss_links_with_articles(Some(query), pool).await?;

    // needs_processing()でフィルタリング
    let processing_links = all_links
        .into_iter()
        .filter(|link| link.needs_processing())
        .collect();

    Ok(processing_links)
}

/// URLから記事内容を取得してArticle構造体に変換する（Firecrawl SDK使用）
pub async fn fetch_article_from_url(url: &str) -> Result<Article> {
    let client =
        ReqwestFirecrawlClient::new().context("実際のFirecrawlクライアントの初期化に失敗")?;
    fetch_article_with_client(url, &client).await
}

/// 指定されたFirecrawlクライアントを使用して記事内容を取得
///
/// この関数は依存注入をサポートし、テスト時にモッククライアントを
/// 注入することでFirecrawl APIへの実際の通信を避けることができます。
pub async fn fetch_article_with_client(url: &str, client: &dyn FirecrawlClient) -> Result<Article> {
    match client.scrape_url(url, None).await {
        Ok(result) => Ok(Article {
            url: url.to_string(),
            timestamp: chrono::Utc::now(),
            status_code: 200,
            content: result
                .markdown
                .unwrap_or_else(|| "記事内容が取得できませんでした".to_string()),
        }),
        Err(e) => Ok(Article {
            url: url.to_string(),
            timestamp: chrono::Utc::now(),
            status_code: 500,
            content: format!("Firecrawl API エラー: {}", e),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::storage::file::load_json_from_file;

    // ファイルからFirecrawlデータを読み込み、Articleに変換する
    fn read_article_from_file(file_path: &str) -> Result<Article> {
        let json_value = load_json_from_file(file_path)?;

        // JSONから必要な値を抽出
        let content = json_value
            .get("markdown")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("markdownフィールドが見つかりません"))?
            .to_string();

        let metadata = json_value
            .get("metadata")
            .ok_or_else(|| anyhow::anyhow!("metadataフィールドが見つかりません"))?;

        // URLを取得（複数の候補から）
        let url = metadata
            .get("url")
            .and_then(|v| v.as_str())
            .or_else(|| metadata.get("sourceURL").and_then(|v| v.as_str()))
            .ok_or_else(|| anyhow::anyhow!("URLが見つかりません"))?
            .to_string();

        // status_codeを取得（必須）
        let status_code = metadata
            .get("statusCode")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .ok_or_else(|| anyhow::anyhow!("statusCodeフィールドが見つかりません"))?;

        let now = Utc::now();

        Ok(Article {
            url,
            timestamp: now,
            status_code,
            content,
        })
    }

    #[test]
    fn test_read_article_from_file() {
        // BBCのモックファイルを読み込んでパース
        let result = read_article_from_file("mock/fc/bbc.json");
        assert!(result.is_ok(), "Firecrawl JSONファイルの読み込みに失敗");

        let article = result.unwrap();

        // 基本的なフィールドの検証
        assert!(!article.content.is_empty(), "contentが空です");
        assert!(!article.url.is_empty(), "URLが空です");

        println!("✅ Firecrawlデータの読み込みテスト成功");
        println!("URL: {}", article.url);
        println!("Contentサイズ: {} characters", article.content.len());
        println!("Status Code: {:?}", article.status_code);
    }

    #[test]
    fn test_read_non_existing_file() {
        // 存在しないファイルを読み込もうとするテスト
        let result = read_article_from_file("non_existent_file.json");
        assert!(result.is_err(), "存在しないファイルでエラーにならなかった");
    }

    #[test]
    fn test_read_article_missing_status_code() {
        // statusCodeが存在しないJSONのテスト
        use std::fs;

        let json_content = r#"
        {
            "markdown": "テスト記事の内容です",
            "metadata": {
                "url": "https://test.example.com/article"
            }
        }
        "#;

        // 一時ファイル作成
        let temp_file = "temp_test_missing_status_code.json";
        fs::write(temp_file, json_content).expect("テストファイルの作成に失敗");

        // statusCodeが存在しない場合にエラーが返されることを確認
        let result = read_article_from_file(temp_file);
        assert!(
            result.is_err(),
            "statusCodeが存在しないのにエラーにならなかった"
        );

        // エラーメッセージの確認
        let error_message = result.unwrap_err().to_string();
        assert!(
            error_message.contains("statusCodeフィールドが見つかりません"),
            "期待されるエラーメッセージが含まれていません: {}",
            error_message
        );

        // テストファイル削除
        fs::remove_file(temp_file).ok();

        println!("✅ statusCode欠損エラーハンドリング検証成功");
    }

    // データベース保存機能のテスト

    // テスト例1: Firecrawl記事の基本的な保存機能のテスト
    #[sqlx::test]
    async fn test_save_article_to_db(pool: PgPool) -> Result<(), anyhow::Error> {
        // テスト用のFirecrawl記事データを作成
        let now = Utc::now();
        let test_article = Article {
            url: "https://test.example.com/firecrawl".to_string(),
            timestamp: now,
            status_code: 200,
            content: "# Test Article\n\nThis is a test content.".to_string(),
        };

        // データベースに保存をテスト
        let result = store_article(&test_article, &pool).await?;

        // SaveResultの検証
        assert_eq!(result.inserted, 1, "新規挿入された記事数が期待と異なります");
        assert_eq!(
            result.skipped_duplicate, 0,
            "重複スキップ数が期待と異なります"
        );

        // 実際にデータベースに保存されたことを確認
        let count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, Some(1), "期待する件数(1件)が保存されませんでした");

        println!("✅ Firecrawl記事保存件数検証成功: {}件", result.inserted);
        println!(
            "✅ Firecrawl SaveResult検証成功: {}",
            result.display_with_domain("Firecrawlドキュメント")
        );

        Ok(())
    }

    // テスト例2: Firecrawl記事の重複処理テスト
    #[sqlx::test]
    async fn test_duplicate_articles(pool: PgPool) -> Result<(), anyhow::Error> {
        let now = Utc::now();

        // 最初の記事を保存
        let original_article = Article {
            url: "https://test.example.com/duplicate".to_string(),
            timestamp: now,
            status_code: 200,
            content: "Original content".to_string(),
        };

        // 最初の記事を保存
        let result1 = store_article(&original_article, &pool).await?;
        assert_eq!(result1.inserted, 1);

        // 同じURLで違う内容の記事を作成（重複）
        let duplicate_article = Article {
            url: "https://test.example.com/duplicate".to_string(),
            timestamp: now,
            status_code: 404,
            content: "Different content".to_string(),
        };

        // 重複記事を保存しようとする（新しい仕様では更新される）
        let result2 = store_article(&duplicate_article, &pool).await?;

        // SaveResultの検証（更新される場合、inserted=1として扱う）
        assert_eq!(result2.inserted, 1, "重複URLの記事は更新されるべきです");
        assert_eq!(
            result2.skipped_duplicate, 0,
            "重複スキップ数が期待と異なります"
        );

        // データベースの件数は1件のまま
        let count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            count,
            Some(1),
            "重複記事が挿入され、件数が変わってしまいました"
        );

        println!("✅ Firecrawl重複スキップ検証成功: {}", result2);

        Ok(())
    }

    // 新機能のテスト
    mod join_functionality_tests {
        use super::*;

        #[test]
        fn test_rss_link_with_article_status_detection() {
            // 未処理リンクのテスト
            let unprocessed = RssLinkWithArticle {
                link: "https://test.com/unprocessed".to_string(),
                title: "未処理記事".to_string(),
                pub_date: Utc::now(),
                article_url: None,
                article_timestamp: None,
                article_status_code: None,
                article_content: None,
            };

            assert!(matches!(
                unprocessed.get_article_status(),
                ArticleStatus::Unprocessed
            ));
            assert!(unprocessed.is_unprocessed());
            assert!(!unprocessed.is_error());
            assert!(unprocessed.needs_processing());

            // 成功記事のテスト
            let success = RssLinkWithArticle {
                link: "https://test.com/success".to_string(),
                title: "成功記事".to_string(),
                pub_date: Utc::now(),
                article_url: Some("https://test.com/success".to_string()),
                article_timestamp: Some(Utc::now()),
                article_status_code: Some(200),
                article_content: Some("記事内容".to_string()),
            };

            assert!(matches!(
                success.get_article_status(),
                ArticleStatus::Success
            ));
            assert!(!success.is_unprocessed());
            assert!(!success.is_error());
            assert!(!success.needs_processing());

            // エラー記事のテスト
            let error = RssLinkWithArticle {
                link: "https://test.com/error".to_string(),
                title: "エラー記事".to_string(),
                pub_date: Utc::now(),
                article_url: Some("https://test.com/error".to_string()),
                article_timestamp: Some(Utc::now()),
                article_status_code: Some(404),
                article_content: Some("エラー内容".to_string()),
            };

            assert!(matches!(
                error.get_article_status(),
                ArticleStatus::Error(404)
            ));
            assert!(!error.is_unprocessed());
            assert!(error.is_error());
            assert!(error.needs_processing());

            println!("✅ RssLinkWithArticle状態判定テスト成功");
        }

        #[sqlx::test]
        async fn test_get_rss_links_with_article_status(pool: PgPool) -> Result<(), anyhow::Error> {
            // テスト用のRSSリンクを挿入
            sqlx::query!(
                "INSERT INTO rss_links (link, title, pub_date) VALUES ($1, $2, CURRENT_TIMESTAMP)",
                "https://test.com/link1",
                "テストリンク1"
            )
            .execute(&pool)
            .await?;

            sqlx::query!(
                "INSERT INTO rss_links (link, title, pub_date) VALUES ($1, $2, CURRENT_TIMESTAMP)",
                "https://test.com/link2",
                "テストリンク2"
            )
            .execute(&pool)
            .await?;

            // 1つのリンクに対応する記事を挿入
            sqlx::query!(
                "INSERT INTO articles (url, status_code, content) VALUES ($1, $2, $3)",
                "https://test.com/link1",
                200,
                "記事内容1"
            )
            .execute(&pool)
            .await?;

            // 全件取得テスト
            let all_links = search_rss_links_with_articles(None, &pool).await?;
            assert!(all_links.len() >= 2, "最低2件のリンクが取得されるべき");

            // link1は記事が紐づいているはず
            let link1 = all_links
                .iter()
                .find(|link| link.link == "https://test.com/link1")
                .expect("link1が見つからない");

            assert!(link1.article_url.is_some(), "link1に記事が紐づいているべき");
            assert_eq!(link1.article_status_code, Some(200));
            assert!(!link1.needs_processing());

            // link2は記事が紐づいていないはず
            let link2 = all_links
                .iter()
                .find(|link| link.link == "https://test.com/link2")
                .expect("link2が見つからない");

            assert!(
                link2.article_url.is_none(),
                "link2に記事が紐づいていないべき"
            );
            assert!(link2.needs_processing());

            println!("✅ JOINクエリテスト成功");
            Ok(())
        }

        #[sqlx::test]
        async fn test_search_unprocessed_rss_links(pool: PgPool) -> Result<(), anyhow::Error> {
            // テスト用のRSSリンクを挿入
            sqlx::query!(
                "INSERT INTO rss_links (link, title, pub_date) VALUES ($1, $2, CURRENT_TIMESTAMP)",
                "https://test.com/unprocessed",
                "未処理リンク"
            )
            .execute(&pool)
            .await?;

            sqlx::query!(
                "INSERT INTO rss_links (link, title, pub_date) VALUES ($1, $2, CURRENT_TIMESTAMP)",
                "https://test.com/processed",
                "処理済みリンク"
            )
            .execute(&pool)
            .await?;

            // 1つだけ記事として処理済みにする
            sqlx::query!(
                "INSERT INTO articles (url, status_code, content) VALUES ($1, $2, $3)",
                "https://test.com/processed",
                200,
                "処理済み記事内容"
            )
            .execute(&pool)
            .await?;

            // 未処理リンクを取得
            let unprocessed_links = search_unprocessed_rss_links(&pool).await?;

            // unprocessedは含まれるべき、processedは含まれないべき
            let unprocessed_urls: Vec<&str> = unprocessed_links
                .iter()
                .map(|link| link.link.as_str())
                .collect();

            assert!(unprocessed_urls.contains(&"https://test.com/unprocessed"));
            assert!(!unprocessed_urls.contains(&"https://test.com/processed"));

            println!("✅ 未処理リンク取得テスト成功");
            Ok(())
        }

        #[sqlx::test]
        async fn test_rss_link_article_query_filters(pool: PgPool) -> Result<(), anyhow::Error> {
            // 複数のテストデータを挿入
            let test_data = vec![
                (
                    "https://example.com/news1",
                    "ニュース1",
                    200,
                    "ニュース1の内容",
                ),
                ("https://example.com/news2", "ニュース2", 404, "エラー内容"),
                (
                    "https://different.com/news3",
                    "ニュース3",
                    200,
                    "ニュース3の内容",
                ),
            ];

            for (url, title, status_code, content) in &test_data {
                sqlx::query!(
                    "INSERT INTO rss_links (link, title, pub_date) VALUES ($1, $2, CURRENT_TIMESTAMP)",
                    url, title
                )
                .execute(&pool)
                .await?;

                sqlx::query!(
                    "INSERT INTO articles (url, status_code, content) VALUES ($1, $2, $3)",
                    url,
                    status_code,
                    content
                )
                .execute(&pool)
                .await?;
            }

            // link_patternフィルターのテスト
            let query = RssLinkArticleQuery {
                link_pattern: Some("example.com".to_string()),
                ..Default::default()
            };
            let example_links = search_rss_links_with_articles(Some(query), &pool).await?;
            assert_eq!(example_links.len(), 2, "example.comのリンクは2件のはず");

            // article_statusフィルターのテスト（成功のみ）
            let query = RssLinkArticleQuery {
                article_status: Some(ArticleStatus::Success),
                ..Default::default()
            };
            let success_links = search_rss_links_with_articles(Some(query), &pool).await?;

            let success_count = success_links
                .iter()
                .filter(|link| link.article_status_code == Some(200))
                .count();
            assert_eq!(
                success_count,
                success_links.len(),
                "成功記事のみが取得されるべき"
            );

            println!("✅ クエリフィルターテスト成功");
            Ok(())
        }

        /// 統一されたFirecrawlテスト - 1つのコードでモック/オンライン切り替え
        #[tokio::test]
        async fn test_fetch_article_unified() -> Result<(), anyhow::Error> {
            use crate::infra::api::firecrawl::MockFirecrawlClient;

            let test_url = "https://httpbin.org/html";
            let mock_content = "統合テスト記事内容\n\nこれは1つのテストコードでモック/オンライン切り替えをテストする記事です。";

            // モッククライアントを使用して統一関数をテスト
            let mock_client = MockFirecrawlClient::new_success(mock_content);
            let article = fetch_article_with_client(test_url, &mock_client).await?;

            // 基本的なアサーション
            assert_eq!(article.url, test_url);
            assert_eq!(article.status_code, 200);
            assert!(article.content.contains(mock_content));

            println!("✅ 統一記事取得テスト成功");
            println!("URL: {}", article.url);
            println!("内容長: {}文字", article.content.len());

            Ok(())
        }

        #[tokio::test]
        async fn test_error_client_handling() -> Result<(), anyhow::Error> {
            // エラークライアントを使用したテスト
            use crate::infra::api::firecrawl::MockFirecrawlClient;

            let error_client = MockFirecrawlClient::new_error("テストエラー");
            let result = fetch_article_with_client("https://test.com", &error_client).await;

            assert!(result.is_ok(), "エラークライアントでも結果を返すべき");

            let article = result.unwrap();
            assert_eq!(
                article.status_code, 500,
                "エラー時はstatus_code=500になるべき"
            );
            assert!(
                article.content.contains("エラー"),
                "エラー内容が記録されるべき"
            );

            println!("✅ エラークライアント処理テスト完了");
            Ok(())
        }
    }
}
