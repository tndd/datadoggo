use crate::infra::api::firecrawl::{FirecrawlClient, ReqwestFirecrawlClient};
use anyhow::{Context, Result};

use super::model::ArticleContent;

/// 外部API(Firecrawl)から記事内容を取得する（本番クライアント）
pub async fn fetch_article_content(url: &str) -> Result<ArticleContent> {
    let client = ReqwestFirecrawlClient::new().context("記事取得クライアントの初期化に失敗")?;
    fetch_article_content_with_client(url, &client).await
}

/// 外部API(Firecrawl)から記事内容を取得する（クライアント注入）
pub async fn fetch_article_content_with_client(
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
            content: format!("記事取得APIエラー: {}", e),
        }),
    }
}

// 互換エイリアスは段階移行完了につき削除済み

#[cfg(test)]
mod tests {
    use crate::infra::{api::firecrawl::MockFirecrawlClient, storage::file::load_json_from_file};

    // helper: モックJSONからArticleContentをざっくり検証
    #[test]
    fn test_mock_json_shape() {
        let json = load_json_from_file("mock/firecrawl/bbc.json").unwrap();
        assert!(json.get("metadata").is_some());
    }

    // オンラインテストはfeatureでガード
    #[cfg(feature = "online")]
    mod online {
        use super::*;

        #[tokio::test]
        async fn test_fetch_article_content() {
            let result = fetch_article_content("https://httpbin.org/html").await;
            assert!(result.is_ok());
        }
    }

    // モックを使った成功・失敗分岐の単体テスト
    mod fetch_with_client {
        use super::*;
        use crate::core::article::fetch_article_content_with_client;

        #[tokio::test]
        async fn test_success() {
            let client = MockFirecrawlClient::new_success("モック成功内容");
            let result = fetch_article_content_with_client("https://example.com", &client)
                .await
                .unwrap();
            assert_eq!(result.status_code, 200);
            assert!(result.content.contains("モック成功内容"));
        }

        #[tokio::test]
        async fn test_error() {
            let client = MockFirecrawlClient::new_error("モック失敗");
            let result = fetch_article_content_with_client("https://example.com", &client)
                .await
                .unwrap();
            assert_eq!(result.status_code, 500);
            assert!(result.content.contains("記事取得APIエラー:"));
        }
    }
}
