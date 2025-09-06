use super::model::{Article, ArticleMetadata, ArticleStatus};
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

// 記事のJOINクエリ用の条件構造体
#[derive(Debug, Default)]
pub struct ArticleQuery {
    pub link_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
    pub article_status: Option<ArticleStatus>,
    pub limit: Option<i64>,
}

// ArticleContent記事のフィルター条件を表す構造体
#[derive(Debug, Default)]
pub struct ArticleContentQuery {
    pub url_pattern: Option<String>,
    pub timestamp_from: Option<DateTime<Utc>>,
    pub timestamp_to: Option<DateTime<Utc>>,
    pub status_code: Option<i32>,
}

/// 記事内容をデータベースに保存する。
/// 重複した場合には更新を行う。
pub async fn store_article_content(article: &ArticleContent, pool: &PgPool) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO articles (url, status_code, content)
        VALUES ($1, $2, $3)
        ON CONFLICT (url) DO UPDATE SET 
            status_code = EXCLUDED.status_code,
            content = EXCLUDED.content,
            timestamp = CURRENT_TIMESTAMP
        WHERE (articles.status_code, articles.content)
            IS DISTINCT FROM (EXCLUDED.status_code, EXCLUDED.content)
        "#,
        article.url,
        article.status_code,
        article.content
    )
    .execute(pool)
    .await
    .context("Firecrawl記事のデータベースへの挿入に失敗しました")?;

    Ok(())
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
            al.url,
            al.title,
            al.pub_date,
            a.timestamp as updated_at,
            a.status_code,
            a.content
        FROM article_links al
        LEFT JOIN articles a ON al.url = a.url
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
        qb.push("al.url ILIKE ").push_bind(pattern);
    }
    // pub_date_from query
    if let Some(pub_date_from) = query.pub_date_from {
        if has_where {
            qb.push(" AND ");
        } else {
            qb.push(" WHERE ");
            has_where = true;
        }
        qb.push("al.pub_date >= ").push_bind(pub_date_from);
    }
    // pub_date_to query
    if let Some(pub_date_to) = query.pub_date_to {
        if has_where {
            qb.push(" AND ");
        } else {
            qb.push(" WHERE ");
            has_where = true;
        }
        qb.push("al.pub_date <= ").push_bind(pub_date_to);
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

    qb.push(" ORDER BY al.pub_date DESC");
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

/// バックログ記事の軽量版を取得する（article_contentを除外し、パフォーマンスを向上）
pub async fn search_backlog_articles_light(
    pool: &PgPool,
    limit: Option<i64>,
) -> Result<Vec<ArticleMetadata>> {
    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        r#"
        SELECT 
            al.url,
            al.title,
            al.pub_date,
            a.timestamp as updated_at,
            a.status_code
        FROM article_links al
        LEFT JOIN articles a ON al.url = a.url
        WHERE a.url IS NULL OR a.status_code != 200
        ORDER BY al.pub_date DESC
        "#,
    );
    // limit
    if let Some(limit) = limit {
        qb.push(" LIMIT ").push_bind(limit);
    }

    let results = qb
        .build_query_as::<ArticleMetadata>()
        .fetch_all(pool)
        .await
        .context("バックログ記事の軽量版取得に失敗")?;

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::storage::file::load_json_from_file;

    // テスト用ヘルパー関数
    mod test_helper {
        use super::*;

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
    }

    // データ永続化・DB操作系テスト
    mod storage {
        use super::*;

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
            store_article_content(&test_article, &pool).await?;
            // 実際にデータベースに保存されたことを確認
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, Some(1), "期待する件数(1件)が保存されませんでした");

            println!("✅ Firecrawl記事保存テスト成功: 1件");

            Ok(())
        }

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
            store_article_content(&original_article, &pool).await?;
            // 同じURLで違う内容の記事内容を作成（重複）
            let duplicate_article = ArticleContent {
                url: "https://test.example.com/duplicate".to_string(),
                timestamp: now,
                status_code: 404,
                content: "Different content".to_string(),
            };
            // 重複記事内容を保存しようとする（新しい仕様では更新される）
            store_article_content(&duplicate_article, &pool).await?;
            // データベースの件数は1件のまま
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                count,
                Some(1),
                "重複記事が挿入され、件数が変わってしまいました"
            );

            println!("✅ Firecrawl重複更新検証成功");

            Ok(())
        }

        // ArticleQueryによるフィルター機能テスト
        #[sqlx::test(fixtures("../../../fixtures/article_query_filter.sql"))]
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

        #[sqlx::test(fixtures("../../../fixtures/article_backlog.sql"))]
        async fn test_search_backlog_articles_light(pool: PgPool) -> Result<(), anyhow::Error> {
            use crate::domain::article::model::{
                count_articles_metadata_by_status, format_backlog_articles_metadata,
            };

            // バックログ記事の軽量版を取得
            let backlog_articles = search_backlog_articles_light(&pool, None).await?;
            // 直接フィールドアクセスで処理
            let backlog_messages = format_backlog_articles_metadata(&backlog_articles);
            let (unprocessed, success, error) =
                count_articles_metadata_by_status(&backlog_articles);
            // 結果の検証
            assert!(backlog_messages.len() >= 2); // 未処理とエラーの両方
            assert!(unprocessed >= 1); // 少なくとも1つの未処理
            assert!(error >= 1); // 少なくとも1つのエラー
            assert_eq!(success, 0); // バックログには成功記事は含まれない

            println!(
                "✅ バックログ軽量版テスト成功: {}件",
                backlog_articles.len()
            );
            Ok(())
        }
    }

    // 複合処理・複数モジュール連携系テスト
    mod composition {
        use super::*;
        use crate::domain::rss::search_backlog_article_links;

        // データベースJOIN機能の統合テスト

        #[sqlx::test(fixtures("../../../fixtures/article_basic.sql"))]
        async fn test_get_articles_status(pool: PgPool) -> Result<(), anyhow::Error> {
            // 全件取得テスト
            let all_links = search_articles(None, &pool).await?;
            assert!(all_links.len() >= 2, "最低2件のリンクが取得されるべき");
            // link1は記事が紐づいているはず
            let link1 = all_links
                .iter()
                .find(|link| link.url == "https://test.com/link1")
                .expect("link1が見つからない");

            assert!(link1.status_code.is_some(), "link1に記事が紐づいているべき");
            assert_eq!(link1.status_code, Some(200));
            assert!(!link1.is_backlog());
            // link2は記事が紐づいていないはず
            let link2 = all_links
                .iter()
                .find(|link| link.url == "https://test.com/link2")
                .expect("link2が見つからない");

            assert!(
                link2.status_code.is_none(),
                "link2に記事が紐づいていないべき"
            );
            assert!(link2.is_backlog());

            println!("✅ JOINクエリテスト成功");
            Ok(())
        }

        #[sqlx::test(fixtures("../../../fixtures/article_unprocessed.sql"))]
        async fn test_search_backlog_rss_links(pool: PgPool) -> Result<(), anyhow::Error> {
            // 未処理リンクを取得
            let unprocessed_links = search_backlog_article_links(&pool).await?;
            // unprocessedは含まれるべき、processedは含まれないべき
            let unprocessed_urls: Vec<&str> = unprocessed_links
                .iter()
                .map(|link| link.url.as_str())
                .collect();

            assert!(unprocessed_urls.contains(&"https://test.com/unprocessed"));
            assert!(!unprocessed_urls.contains(&"https://test.com/processed"));

            println!("✅ 未処理リンク取得テスト成功");
            Ok(())
        }
    }
}
