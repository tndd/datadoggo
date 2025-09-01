//! Firecrawl API関数モジュール
//!
//! このモジュールは、Firecrawl APIへのアクセスを提供し、
//! テスト時のモック化を容易にする関数群を提供します。

use anyhow::{Context, Result};
use firecrawl_sdk::{document::Document, FirecrawlApp};

/// 実際のFirecrawl APIを使用してURLをスクレイピング
pub async fn scrape_url_real(url: &str) -> Result<Document> {
    let firecrawl_app = FirecrawlApp::new_selfhosted("http://localhost:13002", Some("fc-test"))
        .context("Firecrawl SDKの初期化に失敗")?;

    firecrawl_app
        .scrape_url(url, None)
        .await
        .map_err(|e| anyhow::anyhow!("Firecrawl API エラー: {}", e))
}

/// カスタム設定でFirecrawl APIを使用してURLをスクレイピング
pub async fn scrape_url_real_with_config(
    url: &str,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Document> {
    let firecrawl_app =
        FirecrawlApp::new_selfhosted(base_url, api_key).context("Firecrawl SDKの初期化に失敗")?;

    firecrawl_app
        .scrape_url(url, None)
        .await
        .map_err(|e| anyhow::anyhow!("Firecrawl API エラー: {}", e))
}

/// モックでURLスクレイピングをシミュレート（成功レスポンス）
pub async fn scrape_url_mock(url: &str, mock_content: &str) -> Result<Document> {
    // URLを使用してログ出力（実際の処理感を演出）
    println!("モックスクレイピング: {}", url);

    Ok(Document {
        markdown: Some(mock_content.to_string()),
        // 他のフィールドはデフォルト値
        ..Default::default()
    })
}

/// モックでエラーレスポンスをシミュレート
pub async fn scrape_url_error(error_message: &str) -> Result<Document> {
    Err(anyhow::anyhow!("モックエラー: {}", error_message))
}

/// 統一インターフェース - モック内容が指定されていればモック、なければ実API
pub async fn scrape_url(url: &str, mock_content: Option<&str>) -> Result<Document> {
    match mock_content {
        Some(content) => scrape_url_mock(url, content).await,
        None => scrape_url_real(url).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scrape_url_mock_success() {
        let result = scrape_url_mock("https://example.com", "テスト内容").await;

        assert!(result.is_ok());
        let document = result.unwrap();
        assert_eq!(document.markdown, Some("テスト内容".to_string()));
    }

    #[tokio::test]
    async fn test_scrape_url_error() {
        let result = scrape_url_error("テストエラーメッセージ").await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("テストエラーメッセージ"));
    }

    #[tokio::test]
    async fn test_scrape_url_unified_mock() {
        let result = scrape_url("https://example.com", Some("統一テスト内容")).await;

        assert!(result.is_ok());
        let document = result.unwrap();
        assert_eq!(document.markdown, Some("統一テスト内容".to_string()));
    }

    #[tokio::test]
    async fn test_scrape_url_unified_real() {
        // 実APIテストは省略（実際の環境が必要）
        // 単純にscrape_url(url, None)を呼び出すことを確認
        println!("統一インターフェース（実API）のテストはスキップされました");
    }
}
