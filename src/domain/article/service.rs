use super::repository::ArticleContent;
use crate::infra::api::firecrawl::{FirecrawlClient, ReqwestFirecrawlClient};
use anyhow::{Context, Result};

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

#[cfg(test)]
mod tests {
    use super::*;

    // 外部API連携系テスト
    mod external {
        use super::*;

        // Firecrawl記事取得機能の統合テスト

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
}
