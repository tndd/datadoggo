use crate::core::article::model::ArticleContent;
use crate::infra::api::firecrawl::FirecrawlClient;
use anyhow::Result;

/// 外部API(Firecrawl)から記事内容を取得する（クライアント注入）
///
/// これが標準の取得関数です。DIにより `FirecrawlClient` を受け取り、
/// モック/実装を切り替え可能にします。
pub async fn fetch_article_content_with_firecrawl(
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

#[cfg(test)]
mod tests {
    use super::*;

    // オンラインテストはfeatureでガード
    #[cfg(feature = "online")]
    mod online {
        use super::*;
        use crate::infra::api::firecrawl::ReqwestFirecrawlClient;

        /// 実HTTPでの最低限確認
        /// 目的: 実クライアントで `fetch_article_content` が呼べることだけを検証
        #[tokio::test]
        async fn test_fetch_article_content() {
            let client = ReqwestFirecrawlClient::new().expect("クライアント初期化失敗");
            let result =
                fetch_article_content_with_firecrawl("https://httpbin.org/html", &client).await;
            assert!(result.is_ok());
        }
    }

    // モックを使った成功・失敗分岐の単体テスト
    mod fetch_article_content_tests {
        use super::*;
        use crate::infra::api::firecrawl::MockFirecrawlClient;

        /// 成功ケース
        /// 検証観点: status_code=200, contentへモック文字列反映
        #[tokio::test]
        async fn test_success() {
            let client = MockFirecrawlClient::new_success("モック成功内容");
            let result = fetch_article_content_with_firecrawl("https://example.com", &client)
                .await
                .unwrap();
            assert_eq!(result.status_code, 200);
            assert!(result.content.contains("モック成功内容"));
        }

        /// エラーケース
        /// 検証観点: status_code=500, contentにエラーメッセージを含む
        #[tokio::test]
        async fn test_error() {
            let client = MockFirecrawlClient::new_error("モック失敗");
            let result = fetch_article_content_with_firecrawl("https://example.com", &client)
                .await
                .unwrap();
            assert_eq!(result.status_code, 500);
            assert!(result.content.contains("記事取得APIエラー:"));
        }
    }
}
