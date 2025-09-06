use super::model::{Article, ArticleMetadata, ArticleStatus};
use crate::infra::api::firecrawl::{FirecrawlClient, ReqwestFirecrawlClient};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleContent {
    pub url: String,
    pub timestamp: DateTime<Utc>,
    pub status_code: i32,
    pub content: String,
}

#[derive(Debug, Default)]
pub struct ArticleQuery {
    pub link_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
    pub article_status: Option<ArticleStatus>,
    pub limit: Option<i64>,
}

#[derive(Debug, Default)]
pub struct ArticleContentQuery {
    pub url_pattern: Option<String>,
    pub timestamp_from: Option<DateTime<Utc>>,
    pub timestamp_to: Option<DateTime<Utc>>,
    pub status_code: Option<i32>,
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

/// URLから記事を取得してデータベースに保存する統合関数
pub async fn fetch_and_store_article(url: &str, pool: &PgPool) -> Result<ArticleContent> {
    let article = get_article_content(url).await?;
    store_article_content(&article, pool).await?;
    Ok(article)
}

/// 指定されたクライアントを使って記事を取得してデータベースに保存する統合関数（テスト用）
pub async fn fetch_and_store_article_with_client(
    url: &str,
    client: &dyn FirecrawlClient,
    pool: &PgPool,
) -> Result<ArticleContent> {
    let article = get_article_content_with_client(url, client).await?;
    store_article_content(&article, pool).await?;
    Ok(article)
}

/// 指定されたデータベースプールからArticleContentを取得する。
pub async fn search_article_contents(
    query: Option<ArticleContentQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleContent>> {
    let query = query.unwrap_or_default();
    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT url, timestamp, status_code, content FROM articles",
    );

    let mut has_where = false;

    if let Some(ref url_pattern) = query.url_pattern {
        qb.push(" WHERE ");
        has_where = true;
        let url_query = format!("%{}%", url_pattern);
        qb.push("url ILIKE ").push_bind(url_query);
    }

    if let Some(ts_from) = query.timestamp_from {
        if has_where {
            qb.push(" AND ");
        } else {
            qb.push(" WHERE ");
            has_where = true;
        }
        qb.push("timestamp >= ").push_bind(ts_from);
    }

    if let Some(ts_to) = query.timestamp_to {
        if has_where {
            qb.push(" AND ");
        } else {
            qb.push(" WHERE ");
            has_where = true;
        }
        qb.push("timestamp <= ").push_bind(ts_to);
    }

    if let Some(status) = query.status_code {
        if has_where {
            qb.push(" AND ");
        } else {
            qb.push(" WHERE ");
        }
        qb.push("status_code = ").push_bind(status);
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
    if let Some(ref link_pattern) = query.link_pattern {
        if !has_where {
            qb.push(" WHERE ");
            has_where = true;
        }
        let pattern = format!("%{}%", link_pattern);
        qb.push("al.url ILIKE ").push_bind(pattern);
    }
    if let Some(pub_date_from) = query.pub_date_from {
        if has_where {
            qb.push(" AND ");
        } else {
            qb.push(" WHERE ");
            has_where = true;
        }
        qb.push("al.pub_date >= ").push_bind(pub_date_from);
    }
    if let Some(pub_date_to) = query.pub_date_to {
        if has_where {
            qb.push(" AND ");
        } else {
            qb.push(" WHERE ");
            has_where = true;
        }
        qb.push("al.pub_date <= ").push_bind(pub_date_to);
    }
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

    mod helper {
        use super::*;

        fn read_article_content_from_file(file_path: &str) -> Result<ArticleContent> {
            let json_value = load_json_from_file(file_path)?;
            let content = json_value
                .get("markdown")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("markdownフィールドが見つかりません"))?
                .to_string();
            let metadata = json_value
                .get("metadata")
                .ok_or_else(|| anyhow::anyhow!("metadataフィールドが見つかりません"))?;
            let url = metadata
                .get("url")
                .and_then(|v| v.as_str())
                .or_else(|| metadata.get("sourceURL").and_then(|v| v.as_str()))
                .ok_or_else(|| anyhow::anyhow!("URLが見つかりません"))?
                .to_string();
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
            let result = read_article_content_from_file("mock/fc/bbc.json");
            assert!(result.is_ok(), "Firecrawl JSONファイルの読み込みに失敗");

            let article = result.unwrap();
            assert!(!article.content.is_empty(), "contentが空です");
            assert!(!article.url.is_empty(), "URLが空です");

            println!("✅ Firecrawlデータの読み込みテスト成功");
            println!("URL: {}", article.url);
            println!("Contentサイズ: {} characters", article.content.len());
            println!("Status Code: {:?}", article.status_code);
        }

        #[test]
        fn test_read_article_missing_status_code() {
            use std::fs;

            let json_content = r#"
            {
                "markdown": "テスト記事の内容です",
                "metadata": {
                    "url": "https://test.example.com/article"
                }
            }
            "#;
            let temp_file = "temp_test_missing_status_code.json";
            fs::write(temp_file, json_content).expect("テストファイルの作成に失敗");
            let result = read_article_content_from_file(temp_file);
            assert!(
                result.is_err(),
                "statusCodeが存在しないのにエラーにならなかった"
            );
            let error_message = result.unwrap_err().to_string();
            assert!(
                error_message.contains("statusCodeフィールドが見つかりません"),
                "期待されるエラーメッセージが含まれていません: {}",
                error_message
            );
            fs::remove_file(temp_file).ok();

            println!("✅ statusCode欠損エラーハンドリング検証成功");
        }
    }

    mod pure {
        use super::*;

        #[tokio::test]
        async fn test_get_article_content_with_mock() -> Result<(), anyhow::Error> {
            use crate::infra::api::firecrawl::MockFirecrawlClient;

            let test_url = "https://test.com/article";
            let mock_content = "テスト記事内容\n\nこれはモックコンテンツです。";
            let mock_client = MockFirecrawlClient::new_success(mock_content);
            let article = get_article_content_with_client(test_url, &mock_client).await?;

            assert_eq!(article.url, test_url);
            assert_eq!(article.status_code, 200);
            assert!(article.content.contains(mock_content));

            println!("✅ モック記事取得テスト成功");
            Ok(())
        }

        #[tokio::test]
        async fn test_get_article_content_with_error_client() -> Result<(), anyhow::Error> {
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

    mod called {
        use super::*;

        #[sqlx::test]
        async fn test_store_article_content(pool: PgPool) -> Result<(), anyhow::Error> {
            let now = Utc::now();
            let test_article = ArticleContent {
                url: "https://test.example.com/firecrawl".to_string(),
                timestamp: now,
                status_code: 200,
                content: "# Test Article\n\nThis is a test content.".to_string(),
            };
            store_article_content(&test_article, &pool).await?;
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, Some(1), "期待する件数(1件)が保存されませんでした");

            println!("✅ 記事保存テスト成功: 1件");
            Ok(())
        }

        #[sqlx::test]
        async fn test_store_duplicate_article_contents(pool: PgPool) -> Result<(), anyhow::Error> {
            let now = Utc::now();
            let original_article = ArticleContent {
                url: "https://test.example.com/duplicate".to_string(),
                timestamp: now,
                status_code: 200,
                content: "Original content".to_string(),
            };
            store_article_content(&original_article, &pool).await?;
            let duplicate_article = ArticleContent {
                url: "https://test.example.com/duplicate".to_string(),
                timestamp: now,
                status_code: 404,
                content: "Different content".to_string(),
            };
            store_article_content(&duplicate_article, &pool).await?;
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                count,
                Some(1),
                "重複記事が挿入され、件数が変わってしまいました"
            );

            println!("✅ 重複更新検証成功");
            Ok(())
        }

        #[sqlx::test]
        async fn test_search_article_contents(pool: PgPool) -> Result<(), anyhow::Error> {
            let now = Utc::now();
            let test_article = ArticleContent {
                url: "https://search.test.com/article".to_string(),
                timestamp: now,
                status_code: 200,
                content: "検索テスト記事".to_string(),
            };
            store_article_content(&test_article, &pool).await?;

            let query = ArticleContentQuery {
                url_pattern: Some("search.test.com".to_string()),
                ..Default::default()
            };
            let results = search_article_contents(Some(query), &pool).await?;
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].url, "https://search.test.com/article");

            println!("✅ 記事検索テスト成功");
            Ok(())
        }

        #[sqlx::test]
        async fn test_fetch_and_store_article_with_mock(pool: PgPool) -> Result<(), anyhow::Error> {
            use crate::infra::api::firecrawl::MockFirecrawlClient;

            let test_url = "https://integrate.test.com/article";
            let mock_content = "統合テスト記事内容";
            let mock_client = MockFirecrawlClient::new_success(mock_content);

            let article =
                fetch_and_store_article_with_client(test_url, &mock_client, &pool).await?;

            assert_eq!(article.url, test_url);
            assert!(article.content.contains(mock_content));

            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, Some(1));

            println!("✅ 統合機能テスト成功");
            Ok(())
        }

        #[sqlx::test(fixtures("../../../fixtures/article_query_filter.sql"))]
        async fn test_article_query_filters(pool: PgPool) -> Result<(), anyhow::Error> {
            let query = ArticleQuery {
                link_pattern: Some("example.com".to_string()),
                ..Default::default()
            };
            let example_links = search_articles(Some(query), &pool).await?;
            assert_eq!(example_links.len(), 2, "example.comのリンクは2件のはず");

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
            use crate::core::article::model::{
                count_articles_metadata_by_status, format_backlog_articles_metadata,
            };

            let backlog_articles = search_backlog_articles_light(&pool, None).await?;
            let backlog_messages = format_backlog_articles_metadata(&backlog_articles);
            let (unprocessed, success, error) =
                count_articles_metadata_by_status(&backlog_articles);

            assert!(backlog_messages.len() >= 2);
            assert!(unprocessed >= 1);
            assert!(error >= 1);
            assert_eq!(success, 0);

            println!(
                "✅ バックログ軽量版テスト成功: {}件",
                backlog_articles.len()
            );
            Ok(())
        }
    }

    mod online {
        use super::*;

        #[sqlx::test(fixtures("../../../fixtures/article_basic.sql"))]
        async fn test_search_articles_with_join(pool: PgPool) -> Result<(), anyhow::Error> {
            let all_links = search_articles(None, &pool).await?;
            assert!(all_links.len() >= 2, "最低2件のリンクが取得されるべき");

            let link1 = all_links
                .iter()
                .find(|link| link.url == "https://test.com/link1")
                .expect("link1が見つからない");
            assert!(link1.status_code.is_some(), "link1に記事が紐づいているべき");
            assert_eq!(link1.status_code, Some(200));
            assert!(!link1.is_backlog());

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
        async fn test_search_backlog_rss_integration(pool: PgPool) -> Result<(), anyhow::Error> {
            use crate::core::rss::search_backlog_article_links;

            let unprocessed_links = search_backlog_article_links(&pool).await?;
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
