use anyhow::Result;
use sqlx::PgPool;

use super::fetch::{fetch_article_content, fetch_article_content_with_client};
use super::model::ArticleContent;
use crate::infra::api::firecrawl::FirecrawlClient;

/// 記事内容をDBに保存（UPSERT）。
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
    .await?;
    Ok(())
}

/// URLから記事を取得して保存する（本番クライアント）
pub async fn fetch_and_store_article(url: &str, pool: &PgPool) -> Result<ArticleContent> {
    let article = fetch_article_content(url).await?;
    store_article_content(&article, pool).await?;
    Ok(article)
}

/// URLから記事を取得して保存する（クライアント注入）
pub async fn fetch_and_store_article_with_client(
    url: &str,
    client: &dyn FirecrawlClient,
    pool: &PgPool,
) -> Result<ArticleContent> {
    let article = fetch_article_content_with_client(url, client).await?;
    store_article_content(&article, pool).await?;
    Ok(article)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use sqlx::PgPool;

    // 関数名モジュール: store_article_content
    mod store_article_content {
        use super::*;

        #[sqlx::test(fixtures("command_store_article_content_logic"))]
        async fn test_basic_insert_and_conflict_resolution(pool: PgPool) -> anyhow::Result<()> {
            let new_article = ArticleContent {
                url: "https://new.example.com/article".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "New article content".to_string(),
            };
            super::super::store_article_content(&new_article, &pool).await?;

            let count = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM articles WHERE url = $1",
                "https://new.example.com/article"
            )
            .fetch_one(&pool)
            .await?;
            assert_eq!(count.unwrap_or(0), 1);

            let updated_article = ArticleContent {
                url: "https://new.example.com/article".to_string(),
                timestamp: Utc::now(),
                status_code: 404,
                content: "Updated content".to_string(),
            };
            super::super::store_article_content(&updated_article, &pool).await?;

            let (stored_status, stored_content) = sqlx::query!(
                "SELECT status_code, content FROM articles WHERE url = $1",
                "https://new.example.com/article"
            )
            .fetch_one(&pool)
            .await
            .map(|row| (row.status_code, row.content))?;

            assert_eq!(stored_status, 404);
            assert_eq!(stored_content, "Updated content");
            Ok(())
        }

        #[sqlx::test(fixtures("command_store_article_content_logic"))]
        async fn test_distinct_condition_edge_cases(pool: PgPool) -> anyhow::Result<()> {
            let base_url = "https://distinct-test.example.com/article";
            let initial_article = ArticleContent {
                url: base_url.to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "Original content".to_string(),
            };
            super::super::store_article_content(&initial_article, &pool).await?;

            let initial_timestamp =
                sqlx::query_scalar!("SELECT timestamp FROM articles WHERE url = $1", base_url)
                    .fetch_one(&pool)
                    .await?;

            let same_article = ArticleContent {
                url: base_url.to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "Original content".to_string(),
            };
            super::super::store_article_content(&same_article, &pool).await?;
            let timestamp_after_same =
                sqlx::query_scalar!("SELECT timestamp FROM articles WHERE url = $1", base_url)
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(initial_timestamp, timestamp_after_same);

            let status_different = ArticleContent {
                url: base_url.to_string(),
                timestamp: Utc::now(),
                status_code: 404,
                content: "Original content".to_string(),
            };
            super::super::store_article_content(&status_different, &pool).await?;
            let status_after_update =
                sqlx::query_scalar!("SELECT status_code FROM articles WHERE url = $1", base_url)
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(status_after_update, 404);

            let content_different = ArticleContent {
                url: base_url.to_string(),
                timestamp: Utc::now(),
                status_code: 404,
                content: "Modified content".to_string(),
            };
            super::super::store_article_content(&content_different, &pool).await?;
            let content_after_update =
                sqlx::query_scalar!("SELECT content FROM articles WHERE url = $1", base_url)
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(content_after_update, "Modified content");
            Ok(())
        }

        #[sqlx::test(fixtures("command_store_article_content_logic"))]
        async fn test_timestamp_update_behavior(pool: PgPool) -> anyhow::Result<()> {
            let test_url = "https://timestamp-test.example.com/article";
            let initial_time = Utc::now();
            let initial_article = ArticleContent {
                url: test_url.to_string(),
                timestamp: initial_time,
                status_code: 200,
                content: "Initial content".to_string(),
            };
            super::super::store_article_content(&initial_article, &pool).await?;
            let db_timestamp_initial =
                sqlx::query_scalar!("SELECT timestamp FROM articles WHERE url = $1", test_url)
                    .fetch_one(&pool)
                    .await?;

            std::thread::sleep(std::time::Duration::from_millis(10));

            let updated_article = ArticleContent {
                url: test_url.to_string(),
                timestamp: initial_time,
                status_code: 404,
                content: "Initial content".to_string(),
            };
            super::super::store_article_content(&updated_article, &pool).await?;
            let db_timestamp_after_update =
                sqlx::query_scalar!("SELECT timestamp FROM articles WHERE url = $1", test_url)
                    .fetch_one(&pool)
                    .await?;
            assert_ne!(db_timestamp_initial, db_timestamp_after_update);
            assert!(db_timestamp_after_update > db_timestamp_initial);

            let no_change_article = ArticleContent {
                url: test_url.to_string(),
                timestamp: Utc::now(),
                status_code: 404,
                content: "Initial content".to_string(),
            };
            super::super::store_article_content(&no_change_article, &pool).await?;
            let db_timestamp_after_no_change =
                sqlx::query_scalar!("SELECT timestamp FROM articles WHERE url = $1", test_url)
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(db_timestamp_after_update, db_timestamp_after_no_change);
            Ok(())
        }

        #[sqlx::test(fixtures("command_store_article_content_logic"))]
        async fn test_boundary_values_and_data_integrity(pool: PgPool) -> anyhow::Result<()> {
            let empty_content_article = ArticleContent {
                url: "https://empty.example.com/article".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "".to_string(),
            };
            super::super::store_article_content(&empty_content_article, &pool).await?;
            let stored_empty_content = sqlx::query_scalar!(
                "SELECT content FROM articles WHERE url = $1",
                "https://empty.example.com/article"
            )
            .fetch_one(&pool)
            .await?;
            assert_eq!(stored_empty_content, "");

            let boundary_status_codes = vec![0, 100, 200, 404, 500, 599, 999];
            for status_code in boundary_status_codes.iter() {
                let boundary_article = ArticleContent {
                    url: format!("https://status-{}.example.com/article", status_code),
                    timestamp: Utc::now(),
                    status_code: *status_code,
                    content: format!("Content for status {}", status_code),
                };
                super::super::store_article_content(&boundary_article, &pool).await?;
                let stored_status = sqlx::query_scalar!(
                    "SELECT status_code FROM articles WHERE url = $1",
                    boundary_article.url
                )
                .fetch_one(&pool)
                .await?;
                assert_eq!(stored_status, *status_code);
            }

            let long_url = format!(
                "https://very-long-domain-name-for-testing.example.com/{}",
                "a".repeat(200)
            );
            let long_url_article = ArticleContent {
                url: long_url.clone(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "Content for long URL".to_string(),
            };
            super::super::store_article_content(&long_url_article, &pool).await?;
            let stored_url =
                sqlx::query_scalar!("SELECT url FROM articles WHERE url = $1", long_url)
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(stored_url, long_url_article.url);

            let unicode_article = ArticleContent {
                url: "https://unicode.example.com/test".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "日本語🚀<script>alert('test')</script>\n\ttab".to_string(),
            };
            super::super::store_article_content(&unicode_article, &pool).await?;
            let stored_unicode = sqlx::query_scalar!(
                "SELECT content FROM articles WHERE url = $1",
                "https://unicode.example.com/test"
            )
            .fetch_one(&pool)
            .await?;
            assert_eq!(
                stored_unicode,
                "日本語🚀<script>alert('test')</script>\n\ttab",
            );
            Ok(())
        }
    }
}
