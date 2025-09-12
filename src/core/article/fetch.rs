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

// 互換エイリアス（段階移行用）
#[deprecated(note = "fetch_* へ移行してください")]
pub async fn get_article_content(url: &str) -> Result<ArticleContent> {
    fetch_article_content(url).await
}

#[deprecated(note = "fetch_* へ移行してください")]
pub async fn get_article_content_with_client(
    url: &str,
    client: &dyn FirecrawlClient,
) -> Result<ArticleContent> {
    fetch_article_content_with_client(url, client).await
}

#[cfg(test)]
mod tests {
    use crate::infra::storage::file::load_json_from_file;

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
}
