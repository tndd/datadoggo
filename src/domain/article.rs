use crate::infra::api::firecrawl::{FirecrawlClient, ReqwestFirecrawlClient};
use crate::infra::storage::db::DatabaseInsertResult;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

// Firecrawl記事内容構造体（テーブル定義と一致）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleContent {
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

// 記事の共通操作を定義するトレイト
pub trait ArticleView {
    fn get_link(&self) -> &str;
    fn get_title(&self) -> &str;
    fn get_pub_date(&self) -> DateTime<Utc>;
    fn get_status_code(&self) -> Option<i32>;

    // デフォルト実装を提供するメソッド
    fn get_article_status(&self) -> ArticleStatus {
        match self.get_status_code() {
            None => ArticleStatus::Unprocessed,
            Some(200) => ArticleStatus::Success,
            Some(code) => ArticleStatus::Error(code),
        }
    }

    fn is_unprocessed(&self) -> bool {
        matches!(self.get_article_status(), ArticleStatus::Unprocessed)
    }

    fn is_error(&self) -> bool {
        matches!(self.get_article_status(), ArticleStatus::Error(_))
    }

    fn is_backlog(&self) -> bool {
        self.is_unprocessed() || self.is_error()
    }
}

// 記事エンティティ（RSSリンクと記事内容の統合表現）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Article {
    pub link: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub status_code: Option<i32>,
    pub content: Option<String>,
}

// 軽量記事エンティティ（バックログ処理用、contentを除外）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleLight {
    pub link: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub status_code: Option<i32>,
    // content フィールドは意図的に除外
}

// ArticleViewトレイトの実装
impl ArticleView for Article {
    fn get_link(&self) -> &str {
        &self.link
    }
    fn get_title(&self) -> &str {
        &self.title
    }
    fn get_pub_date(&self) -> DateTime<Utc> {
        self.pub_date
    }
    fn get_status_code(&self) -> Option<i32> {
        self.status_code
    }
}

impl ArticleView for ArticleLight {
    fn get_link(&self) -> &str {
        &self.link
    }
    fn get_title(&self) -> &str {
        &self.title
    }
    fn get_pub_date(&self) -> DateTime<Utc> {
        self.pub_date
    }
    fn get_status_code(&self) -> Option<i32> {
        self.status_code
    }
}

// 既存のAPIとの互換性のため、Articleに直接メソッドも追加
impl Article {
    /// 記事の処理状態を取得
    pub fn get_article_status(&self) -> ArticleStatus {
        ArticleView::get_article_status(self)
    }
    /// 未処理のリンクかどうかを判定
    pub fn is_unprocessed(&self) -> bool {
        ArticleView::is_unprocessed(self)
    }
    /// エラー状態のリンクかどうかを判定
    pub fn is_error(&self) -> bool {
        ArticleView::is_error(self)
    }
    /// 処理が必要なリンクかどうかを判定（未処理またはエラー）
    pub fn is_backlog(&self) -> bool {
        ArticleView::is_backlog(self)
    }
}

// 記事のJOINクエリ用の条件構造体
#[derive(Debug, Default)]
pub struct ArticleQuery {
    pub link_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
    pub article_status: Option<ArticleStatus>,
    pub limit: Option<i64>,
}

/// 記事内容をデータベースに保存する。
/// 重複した場合には更新を行う。
pub async fn store_article_content(
    article: &ArticleContent,
    pool: &PgPool,
) -> Result<DatabaseInsertResult> {
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

// ArticleContent記事のフィルター条件を表す構造体
#[derive(Debug, Default)]
pub struct ArticleContentQuery {
    pub url_pattern: Option<String>,
    pub timestamp_from: Option<DateTime<Utc>>,
    pub timestamp_to: Option<DateTime<Utc>>,
    pub status_code: Option<i32>,
}

/// 指定されたデータベースプールからArticleContentを取得する。
pub async fn search_article_contents(
    query: Option<ArticleContentQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleContent>> {
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

    let articles = qb
        .build_query_as::<ArticleContent>()
        .fetch_all(pool)
        .await?;

    Ok(articles)
}

/// RSSリンクと記事の結合情報を取得する
pub async fn search_articles(query: Option<ArticleQuery>, pool: &PgPool) -> Result<Vec<Article>> {
    let query = query.unwrap_or_default();

    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        r#"
        SELECT 
            rl.link,
            rl.title,
            rl.pub_date,
            a.timestamp as updated_at,
            a.status_code,
            a.content
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
        .build_query_as::<Article>()
        .fetch_all(pool)
        .await
        .context("記事情報の取得に失敗")?;

    Ok(results)
}

/// URLから記事内容を取得してArticleContent構造体に変換する（Firecrawl SDK使用）
pub async fn get_article_content(url: &str) -> Result<ArticleContent> {
    let client =
        ReqwestFirecrawlClient::new().context("実際のFirecrawlクライアントの初期化に失敗")?;
    get_article_content_with_client(url, &client).await
}

/// 指定されたFirecrawlクライアントを使用して記事内容を取得
///
/// この関数は依存注入をサポートし、テスト時にモッククライアントを
/// 注入することでFirecrawl APIへの実際の通信を避けることができます。
pub async fn get_article_content_with_client(
    url: &str,
    client: &dyn FirecrawlClient,
) -> Result<ArticleContent> {
    match client.scrape_url(url).await {
        Ok(result) => Ok(ArticleContent {
            url: url.to_string(),
            timestamp: chrono::Utc::now(),
            status_code: 200,
            content: result
                .markdown
                .unwrap_or_else(|| "記事内容が取得できませんでした".to_string()),
        }),
        Err(e) => Ok(ArticleContent {
            url: url.to_string(),
            timestamp: chrono::Utc::now(),
            status_code: 500,
            content: format!("Firecrawl API エラー: {}", e),
        }),
    }
}

/// バックログ記事の軽量版を取得する（article_contentを除外し、パフォーマンスを向上）
pub async fn search_backlog_articles_light(
    pool: &PgPool,
    limit: Option<i64>,
) -> Result<Vec<ArticleLight>> {
    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        r#"
        SELECT 
            rl.link,
            rl.title,
            rl.pub_date,
            a.timestamp as updated_at,
            a.status_code
        FROM rss_links rl
        LEFT JOIN articles a ON rl.link = a.url
        WHERE a.url IS NULL OR a.status_code != 200
        ORDER BY rl.pub_date DESC
        "#,
    );
    // limit
    if let Some(limit) = limit {
        qb.push(" LIMIT ").push_bind(limit);
    }

    let results = qb
        .build_query_as::<ArticleLight>()
        .fetch_all(pool)
        .await
        .context("バックログ記事の軽量版取得に失敗")?;

    Ok(results)
}

/// ArticleViewトレイトを使用したジェネリック処理関数
pub fn format_backlog_articles<T: ArticleView>(articles: &[T]) -> Vec<String> {
    articles
        .iter()
        .filter(|article| article.is_backlog())
        .map(|article| format!("処理待ち: {} - {}", article.get_title(), article.get_link()))
        .collect()
}

/// 記事ステータスでフィルタリングするジェネリック関数
pub fn filter_articles_by_status<T: ArticleView>(articles: &[T], status: ArticleStatus) -> Vec<&T> {
    articles
        .iter()
        .filter(|article| match status {
            ArticleStatus::Unprocessed => article.is_unprocessed(),
            ArticleStatus::Success => {
                matches!(article.get_article_status(), ArticleStatus::Success)
            }
            ArticleStatus::Error(code) => {
                matches!(article.get_article_status(), ArticleStatus::Error(c) if c == code)
            }
        })
        .collect()
}

/// 記事統計情報を計算するジェネリック関数
pub fn count_articles_by_status<T: ArticleView>(articles: &[T]) -> (usize, usize, usize) {
    let mut unprocessed = 0;
    let mut success = 0;
    let mut error = 0;

    for article in articles {
        match article.get_article_status() {
            ArticleStatus::Unprocessed => unprocessed += 1,
            ArticleStatus::Success => success += 1,
            ArticleStatus::Error(_) => error += 1,
        }
    }

    (unprocessed, success, error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::storage::file::load_json_from_file;

    // ファイルからFirecrawlデータを読み込み、ArticleContentに変換する
    fn read_article_content_from_file(file_path: &str) -> Result<ArticleContent> {
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

        Ok(ArticleContent {
            url,
            timestamp: now,
            status_code,
            content,
        })
    }

    #[test]
    fn test_read_article_from_file() {
        // BBCのモックファイルを読み込んでパース
        let result = read_article_content_from_file("mock/fc/bbc.json");
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
        let result = read_article_content_from_file(temp_file);
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

    // テスト例1: Firecrawl記事内容の基本的な保存機能のテスト
    #[sqlx::test]
    async fn test_save_article_content_to_db(pool: PgPool) -> Result<(), anyhow::Error> {
        // テスト用のFirecrawl記事内容データを作成
        let now = Utc::now();
        let test_article = ArticleContent {
            url: "https://test.example.com/firecrawl".to_string(),
            timestamp: now,
            status_code: 200,
            content: "# Test Article\n\nThis is a test content.".to_string(),
        };
        // データベースに保存をテスト
        let result = store_article_content(&test_article, &pool).await?;
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

    // テスト例2: Firecrawl記事内容の重複処理テスト
    #[sqlx::test]
    async fn test_duplicate_article_contents(pool: PgPool) -> Result<(), anyhow::Error> {
        let now = Utc::now();
        // 最初の記事内容を保存
        let original_article = ArticleContent {
            url: "https://test.example.com/duplicate".to_string(),
            timestamp: now,
            status_code: 200,
            content: "Original content".to_string(),
        };
        // 最初の記事内容を保存
        let result1 = store_article_content(&original_article, &pool).await?;
        assert_eq!(result1.inserted, 1);
        // 同じURLで違う内容の記事内容を作成（重複）
        let duplicate_article = ArticleContent {
            url: "https://test.example.com/duplicate".to_string(),
            timestamp: now,
            status_code: 404,
            content: "Different content".to_string(),
        };
        // 重複記事内容を保存しようとする（新しい仕様では更新される）
        let result2 = store_article_content(&duplicate_article, &pool).await?;
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

    // 記事ステータス判定機能のテスト
    mod article_status_tests {
        use super::*;

        #[test]
        fn test_article_status_detection() {
            // 未処理リンクのテスト
            let unprocessed = Article {
                link: "https://test.com/unprocessed".to_string(),
                title: "未処理記事".to_string(),
                pub_date: Utc::now(),
                updated_at: None,
                status_code: None,
                content: None,
            };
            assert!(matches!(
                unprocessed.get_article_status(),
                ArticleStatus::Unprocessed
            ));
            assert!(unprocessed.is_unprocessed());
            assert!(!unprocessed.is_error());
            assert!(unprocessed.is_backlog());
            // 成功記事のテスト
            let success = Article {
                link: "https://test.com/success".to_string(),
                title: "成功記事".to_string(),
                pub_date: Utc::now(),
                updated_at: Some(Utc::now()),
                status_code: Some(200),
                content: Some("記事内容".to_string()),
            };
            assert!(matches!(
                success.get_article_status(),
                ArticleStatus::Success
            ));
            assert!(!success.is_unprocessed());
            assert!(!success.is_error());
            assert!(!success.is_backlog());
            // エラー記事のテスト
            let error = Article {
                link: "https://test.com/error".to_string(),
                title: "エラー記事".to_string(),
                pub_date: Utc::now(),
                updated_at: Some(Utc::now()),
                status_code: Some(404),
                content: Some("エラー内容".to_string()),
            };
            assert!(matches!(
                error.get_article_status(),
                ArticleStatus::Error(404)
            ));
            assert!(!error.is_unprocessed());
            assert!(error.is_error());
            assert!(error.is_backlog());

            println!("✅ Article状態判定テスト成功");
        }
    }

    // データベースJOIN機能の統合テスト
    mod database_integration_tests {
        use super::*;
        use crate::domain::rss::search_unprocessed_rss_links;

        #[sqlx::test(fixtures("../../fixtures/article_basic.sql"))]
        async fn test_get_articles_status(pool: PgPool) -> Result<(), anyhow::Error> {
            // 全件取得テスト
            let all_links = search_articles(None, &pool).await?;
            assert!(all_links.len() >= 2, "最低2件のリンクが取得されるべき");
            // link1は記事が紐づいているはず
            let link1 = all_links
                .iter()
                .find(|link| link.link == "https://test.com/link1")
                .expect("link1が見つからない");

            assert!(link1.status_code.is_some(), "link1に記事が紐づいているべき");
            assert_eq!(link1.status_code, Some(200));
            assert!(!link1.is_backlog());
            // link2は記事が紐づいていないはず
            let link2 = all_links
                .iter()
                .find(|link| link.link == "https://test.com/link2")
                .expect("link2が見つからない");

            assert!(
                link2.status_code.is_none(),
                "link2に記事が紐づいていないべき"
            );
            assert!(link2.is_backlog());

            println!("✅ JOINクエリテスト成功");
            Ok(())
        }

        #[sqlx::test(fixtures("../../fixtures/article_unprocessed.sql"))]
        async fn test_search_unprocessed_rss_links(pool: PgPool) -> Result<(), anyhow::Error> {
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
    }

    // ArticleQueryによるフィルター機能テスト
    mod query_filter_tests {
        use super::*;

        #[sqlx::test(fixtures("../../fixtures/article_query_filter.sql"))]
        async fn test_article_query_filters(pool: PgPool) -> Result<(), anyhow::Error> {
            // link_patternフィルターのテスト
            let query = ArticleQuery {
                link_pattern: Some("example.com".to_string()),
                ..Default::default()
            };
            let example_links = search_articles(Some(query), &pool).await?;
            assert_eq!(example_links.len(), 2, "example.comのリンクは2件のはず");
            // article_statusフィルターのテスト（成功のみ）
            let query = ArticleQuery {
                article_status: Some(ArticleStatus::Success),
                ..Default::default()
            };
            let success_links = search_articles(Some(query), &pool).await?;

            let success_count = success_links
                .iter()
                .filter(|link| link.status_code == Some(200))
                .count();
            assert_eq!(
                success_count,
                success_links.len(),
                "成功記事のみが取得されるべき"
            );

            println!("✅ クエリフィルターテスト成功");
            Ok(())
        }
    }

    // Firecrawl記事取得機能の統合テスト
    mod firecrawl_integration_tests {
        use super::*;

        /// 統一されたFirecrawlテスト - 1つのコードでモック/オンライン切り替え
        #[tokio::test]
        async fn test_fetch_article_unified() -> Result<(), anyhow::Error> {
            use crate::infra::api::firecrawl::MockFirecrawlClient;

            let test_url = "https://httpbin.org/html";
            let mock_content = "統合テスト記事内容\n\nこれは1つのテストコードでモック/オンライン切り替えをテストする記事です。";
            // モッククライアントを使用して統一関数をテスト
            let mock_client = MockFirecrawlClient::new_success(mock_content);
            let article = get_article_content_with_client(test_url, &mock_client).await?;
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
            let result = get_article_content_with_client("https://test.com", &error_client).await;

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

    // ArticleViewトレイトとジェネリック処理のテスト
    mod article_view_trait_tests {
        use super::*;

        #[test]
        fn test_article_view_trait_implementation() {
            // 完全版記事のテスト
            let full_article = Article {
                link: "https://test.com/full".to_string(),
                title: "完全版記事".to_string(),
                pub_date: Utc::now(),
                updated_at: Some(Utc::now()),
                status_code: Some(200),
                content: Some("記事内容".to_string()),
            };
            // 軽量版記事のテスト
            let light_article = ArticleLight {
                link: "https://test.com/light".to_string(),
                title: "軽量版記事".to_string(),
                pub_date: Utc::now(),
                updated_at: Some(Utc::now()),
                status_code: Some(404),
            };
            // ArticleViewトレイト経由でのアクセス
            assert_eq!(full_article.get_link(), "https://test.com/full");
            assert_eq!(full_article.get_title(), "完全版記事");
            assert_eq!(full_article.get_status_code(), Some(200));
            assert!(!full_article.is_backlog());

            assert_eq!(light_article.get_link(), "https://test.com/light");
            assert_eq!(light_article.get_title(), "軽量版記事");
            assert_eq!(light_article.get_status_code(), Some(404));
            assert!(light_article.is_backlog());

            println!("✅ ArticleViewトレイト実装テスト成功");
        }

        #[test]
        fn test_generic_functions() {
            let full_articles = vec![
                Article {
                    link: "https://test.com/success".to_string(),
                    title: "成功記事".to_string(),
                    pub_date: Utc::now(),
                    updated_at: Some(Utc::now()),
                    status_code: Some(200),
                    content: Some("成功内容".to_string()),
                },
                Article {
                    link: "https://test.com/error".to_string(),
                    title: "エラー記事".to_string(),
                    pub_date: Utc::now(),
                    updated_at: Some(Utc::now()),
                    status_code: Some(404),
                    content: Some("エラー内容".to_string()),
                },
            ];

            let light_articles = vec![
                ArticleLight {
                    link: "https://test.com/unprocessed".to_string(),
                    title: "未処理記事".to_string(),
                    pub_date: Utc::now(),
                    updated_at: None,
                    status_code: None,
                },
                ArticleLight {
                    link: "https://test.com/success_light".to_string(),
                    title: "成功軽量記事".to_string(),
                    pub_date: Utc::now(),
                    updated_at: Some(Utc::now()),
                    status_code: Some(200),
                },
            ];
            // ジェネリック処理関数のテスト
            let full_backlog = format_backlog_articles(&full_articles);
            let light_backlog = format_backlog_articles(&light_articles);

            assert_eq!(full_backlog.len(), 1);
            assert!(full_backlog[0].contains("エラー記事"));
            assert_eq!(light_backlog.len(), 1);
            assert!(light_backlog[0].contains("未処理記事"));
            // ステータスフィルタリングのテスト
            let error_articles =
                filter_articles_by_status(&full_articles, ArticleStatus::Error(404));
            assert_eq!(error_articles.len(), 1);
            assert_eq!(error_articles[0].get_title(), "エラー記事");

            let success_light = filter_articles_by_status(&light_articles, ArticleStatus::Success);
            assert_eq!(success_light.len(), 1);
            assert_eq!(success_light[0].get_title(), "成功軽量記事");
            // 統計計算のテスト
            let (unprocessed, success, error) = count_articles_by_status(&full_articles);
            assert_eq!((unprocessed, success, error), (0, 1, 1));

            let (light_unprocessed, light_success, light_error) =
                count_articles_by_status(&light_articles);
            assert_eq!((light_unprocessed, light_success, light_error), (1, 1, 0));

            println!("✅ ジェネリック関数テスト成功");
        }

        #[sqlx::test(fixtures("../../fixtures/article_backlog.sql"))]
        async fn test_search_backlog_articles_light(pool: PgPool) -> Result<(), anyhow::Error> {
            // バックログ記事の軽量版を取得
            let backlog_articles = search_backlog_articles_light(&pool, None).await?;
            // トレイトを使って処理
            let backlog_messages = format_backlog_articles(&backlog_articles);
            let (unprocessed, success, error) = count_articles_by_status(&backlog_articles);
            // 結果の検証
            assert!(backlog_messages.len() >= 2); // 未処理とエラーの両方
            assert!(unprocessed >= 1); // 少なくとも1つの未処理
            assert!(error >= 1); // 少なくとも1つのエラー
            assert_eq!(success, 0); // バックログには成功記事は含まれない

            println!(
                "✅ トレイトベース バックログ軽量版テスト成功: {}件",
                backlog_articles.len()
            );
            Ok(())
        }
    }
}
