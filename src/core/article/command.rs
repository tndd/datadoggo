use anyhow::Result;
use sqlx::PgPool;

use super::fetch::fetch_article_content_via_firecrawl;
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

/// URLから記事を取得して保存する（クライアント注入）
///
/// DI版を正とし、常に `FirecrawlClient` を注入する。
pub async fn fetch_via_firecrawl_and_store_article_content(
    url: &str,
    client: &dyn FirecrawlClient,
    pool: &PgPool,
) -> Result<ArticleContent> {
    let article = fetch_article_content_via_firecrawl(url, client).await?;
    store_article_content(&article, pool).await?;
    Ok(article)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use sqlx::PgPool;

    mod store_article_content {
        use super::*;

        /// 基本的な新規挿入テスト
        /// 目的: 空のテーブルへの記事挿入動作を確認
        #[sqlx::test]
        async fn test_basic_insert(pool: PgPool) -> Result<()> {
            let article = ArticleContent {
                url: "https://new.com/article".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "New article content".to_string(),
            };

            store_article_content(&article, &pool).await?;

            // DB確認
            let row = sqlx::query!(
                "SELECT url, status_code, content FROM articles WHERE url = $1",
                article.url
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(row.url, article.url);
            assert_eq!(Some(row.status_code), Some(article.status_code));
            assert_eq!(Some(row.content), Some(article.content));

            Ok(())
        }

        /// 重複時の更新テスト（内容変更時）
        /// 目的: DISTINCT FROM条件による更新動作を確認
        #[sqlx::test(fixtures("command"))]
        async fn test_update_on_content_change(pool: PgPool) -> Result<()> {
            let updated_article = ArticleContent {
                url: "https://update.com/article".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "Updated content".to_string(),
            };

            // 更新前のタイムスタンプ取得
            let before_row = sqlx::query!(
                "SELECT timestamp FROM articles WHERE url = $1",
                updated_article.url
            )
            .fetch_one(&pool)
            .await?;

            store_article_content(&updated_article, &pool).await?;

            // 更新後確認
            let after_row = sqlx::query!(
                "SELECT status_code, content, timestamp FROM articles WHERE url = $1",
                updated_article.url
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(
                Some(after_row.status_code),
                Some(updated_article.status_code)
            );
            assert_eq!(Some(after_row.content), Some(updated_article.content));

            // タイムスタンプが更新されていることを確認
            assert!(after_row.timestamp > before_row.timestamp);

            Ok(())
        }

        /// 重複時の更新スキップテスト（同内容時）
        /// 目的: DISTINCT FROM条件による更新スキップを確認
        #[sqlx::test(fixtures("command"))]
        async fn test_skip_update_on_same_content(pool: PgPool) -> Result<()> {
            let same_article = ArticleContent {
                url: "https://existing.com/article".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "Original content".to_string(), // 元と同じ内容
            };

            // 更新前のタイムスタンプ取得
            let before_row = sqlx::query!(
                "SELECT timestamp FROM articles WHERE url = $1",
                same_article.url
            )
            .fetch_one(&pool)
            .await?;

            store_article_content(&same_article, &pool).await?;

            // 更新後確認
            let after_row = sqlx::query!(
                "SELECT timestamp FROM articles WHERE url = $1",
                same_article.url
            )
            .fetch_one(&pool)
            .await?;

            // タイムスタンプが変更されていないことを確認（DISTINCT FROM条件でスキップされた）
            assert_eq!(after_row.timestamp, before_row.timestamp);

            Ok(())
        }

        /// ステータスコード変更時の更新テスト
        /// 目的: status_codeの変更でも更新が実行されることを確認
        #[sqlx::test(fixtures("command"))]
        async fn test_update_on_status_change(pool: PgPool) -> Result<()> {
            let status_changed_article = ArticleContent {
                url: "https://same.com/article".to_string(),
                timestamp: Utc::now(),
                status_code: 500,                     // 404から500に変更
                content: "Error content".to_string(), // 内容は同じ
            };

            // 更新前のタイムスタンプ取得
            let before_row = sqlx::query!(
                "SELECT timestamp FROM articles WHERE url = $1",
                status_changed_article.url
            )
            .fetch_one(&pool)
            .await?;

            store_article_content(&status_changed_article, &pool).await?;

            // 更新後確認
            let after_row = sqlx::query!(
                "SELECT status_code, timestamp FROM articles WHERE url = $1",
                status_changed_article.url
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(Some(after_row.status_code), Some(500));
            // ステータス変更でもタイムスタンプが更新されることを確認
            assert!(after_row.timestamp > before_row.timestamp);

            Ok(())
        }

        /// 複数記事の連続挿入テスト
        /// 目的: 複数の記事を連続で処理する動作を確認
        #[sqlx::test]
        async fn test_multiple_inserts(pool: PgPool) -> Result<()> {
            let articles = vec![
                ArticleContent {
                    url: "https://multi1.com/article".to_string(),
                    timestamp: Utc::now(),
                    status_code: 200,
                    content: "Content 1".to_string(),
                },
                ArticleContent {
                    url: "https://multi2.com/article".to_string(),
                    timestamp: Utc::now(),
                    status_code: 404,
                    content: "Content 2".to_string(),
                },
            ];

            for article in &articles {
                store_article_content(article, &pool).await?;
            }

            // 全件確認
            let count = sqlx::query!("SELECT COUNT(*) as count FROM articles")
                .fetch_one(&pool)
                .await?;

            assert_eq!(count.count, Some(2));

            Ok(())
        }
    }

    mod fetch_and_store_article {
        use super::*;
        use crate::infra::api::firecrawl::MockFirecrawlClient;

        /// 成功ケースでの記事取得と保存テスト
        /// 目的: MockFirecrawlClientでの成功処理とDB保存を確認
        #[sqlx::test]
        async fn test_fetch_and_store_success(pool: PgPool) -> Result<()> {
            let client = MockFirecrawlClient::new_success("モック記事内容");
            let url = "https://success.com/article";

            let result = fetch_via_firecrawl_and_store_article_content(url, &client, &pool).await?;

            // 戻り値確認
            assert_eq!(result.url, url);
            assert_eq!(result.status_code, 200);
            assert!(result.content.contains("モック記事内容"));

            // DB保存確認
            let row = sqlx::query!(
                "SELECT url, status_code, content FROM articles WHERE url = $1",
                url
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(row.url, url);
            assert_eq!(Some(row.status_code), Some(200));
            assert!(row.content.contains("モック記事内容"));

            Ok(())
        }

        /// エラーケースでの記事取得と保存テスト
        /// 目的: MockFirecrawlClientでのエラー処理とDB保存を確認
        #[sqlx::test]
        async fn test_fetch_and_store_error(pool: PgPool) -> Result<()> {
            let client = MockFirecrawlClient::new_error("取得エラー");
            let url = "https://error.com/article";

            let result = fetch_via_firecrawl_and_store_article_content(url, &client, &pool).await?;

            // 戻り値確認（エラー時はstatus_code=500）
            assert_eq!(result.url, url);
            assert_eq!(result.status_code, 500);
            assert!(result.content.contains("記事取得APIエラー:"));

            // DB保存確認
            let row = sqlx::query!(
                "SELECT url, status_code, content FROM articles WHERE url = $1",
                url
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(row.url, url);
            assert_eq!(Some(row.status_code), Some(500));
            assert!(row.content.contains("記事取得APIエラー:"));

            Ok(())
        }

        /// 複数URLの連続処理テスト
        /// 目的: 複数のURLを連続で取得・保存する動作を確認
        #[sqlx::test]
        async fn test_multiple_fetch_and_store(pool: PgPool) -> Result<()> {
            let success_client = MockFirecrawlClient::new_success("成功内容");
            let error_client = MockFirecrawlClient::new_error("失敗内容");

            let urls = vec![
                ("https://multi1.com/article", &success_client),
                ("https://multi2.com/article", &error_client),
            ];

            for (url, client) in urls {
                let _result =
                    fetch_via_firecrawl_and_store_article_content(url, client, &pool).await?;
            }

            // 全件確認
            let count = sqlx::query!("SELECT COUNT(*) as count FROM articles")
                .fetch_one(&pool)
                .await?;

            assert_eq!(count.count, Some(2));

            // 成功・エラー両方が保存されていることを確認
            let success_row = sqlx::query!(
                "SELECT status_code FROM articles WHERE url = $1",
                "https://multi1.com/article"
            )
            .fetch_one(&pool)
            .await?;
            assert_eq!(Some(success_row.status_code), Some(200));

            let error_row = sqlx::query!(
                "SELECT status_code FROM articles WHERE url = $1",
                "https://multi2.com/article"
            )
            .fetch_one(&pool)
            .await?;
            assert_eq!(Some(error_row.status_code), Some(500));

            Ok(())
        }
    }

    // オンラインテスト（実際のHTTP通信）
    #[cfg(feature = "online")]
    mod online {
        use super::*;
        use crate::infra::api::firecrawl::ReqwestFirecrawlClient;

        mod fetch_and_store_article {
            use super::*;

            /// 実際のHTTP通信での記事取得テスト
            /// 目的: 本番環境での動作確認（onlineフィーチャーフラグ必須）
            /// 注意: このテストは通常のテスト実行時には実行されない
            #[sqlx::test]
            async fn test_online_fetch_and_store(pool: PgPool) -> Result<()> {
                // 実在のテスト用URL（レスポンス保証のあるサイト）
                let url = "https://httpbin.org/html";

                let client = ReqwestFirecrawlClient::new()?;
                let result = fetch_and_store_article(url, &client, &pool).await?;

                // 基本的な確認のみ（内容は不安定なため）
                assert_eq!(result.url, url);
                assert!(!result.content.is_empty());

                // DB保存確認
                let row = sqlx::query!("SELECT url FROM articles WHERE url = $1", url)
                    .fetch_one(&pool)
                    .await?;

                assert_eq!(row.url, url);

                Ok(())
            }
        }
    }
}
