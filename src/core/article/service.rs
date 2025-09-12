// 共有ユーティリティをここに集約（article配下でのみ共有）
use super::model::{ArticleContent, ArticleStatus};
use crate::infra::api::firecrawl::{FirecrawlClient, ReqwestFirecrawlClient};
use anyhow::{Context, Result};

/// 内部実装：statuses指定の正規化を行う
/// queryのSQLバインド補助。記事検索系でのみ利用するため公開は限定。
pub(super) fn normalize_statuses(
    statuses: Option<&[ArticleStatus]>,
) -> (bool, bool, bool, Option<Vec<i32>>) {
    let mut apply = false;
    let mut has_unprocessed = false;
    let mut has_success = false;
    let mut errors: Vec<i32> = Vec::new();

    if let Some(list) = statuses {
        for s in list {
            apply = true;
            match s {
                ArticleStatus::Unprocessed => has_unprocessed = true,
                ArticleStatus::Success => has_success = true,
                ArticleStatus::Error(code) => errors.push(*code),
            }
        }
    }

    let error_codes = if errors.is_empty() {
        None
    } else {
        Some(errors)
    };
    (apply, has_unprocessed, has_success, error_codes)
}

// ============
//    Fetch
// ============

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_statuses() {
        let (apply, unp, ok, errs) = normalize_statuses(Some(&[
            ArticleStatus::Unprocessed,
            ArticleStatus::Success,
            ArticleStatus::Error(404),
            ArticleStatus::Error(500),
        ]));
        assert!(apply);
        assert!(unp);
        assert!(ok);
        assert_eq!(errs.unwrap(), vec![404, 500]);
    }

    // fetch系テスト（fetch.rsから移設）
    use crate::infra::storage::file::load_json_from_file;

    #[test]
    fn test_mock_json_shape() {
        // モックJSONの基本構造をざっくり検証
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
        use crate::infra::api::firecrawl::MockFirecrawlClient;

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
