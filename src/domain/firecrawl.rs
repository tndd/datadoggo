//! Firecrawl クライアント抽象化モジュール
//!
//! このモジュールは、Firecrawl APIへのアクセスを抽象化し、
//! テスト時のモック化を容易にするプロトコルとその実装を提供します。

use anyhow::{Context, Result};
use async_trait::async_trait;
use firecrawl_sdk::{document::Document, FirecrawlApp};

/// Firecrawl APIの抽象化プロトコル
///
/// このプロトコルは、実際のFirecrawl APIとモック実装の両方を
/// 統一的に扱えるようにするためのインターフェースです。
#[async_trait]
pub trait FirecrawlClientProtocol {
    /// URLをスクレイピングして結果を返す
    ///
    /// # Arguments
    /// * `url` - スクレイピング対象のURL
    /// * `options` - スクレイピングオプション（現在はNoneのみ対応）
    async fn scrape_url(&self, url: &str, options: Option<()>) -> Result<Document>;
}

/// 実際のFirecrawl APIを使用する実装
pub struct FirecrawlClient {
    firecrawl_app: FirecrawlApp,
}

impl FirecrawlClient {
    /// デフォルトのFirecrawl設定で新しいクライアントを作成
    pub fn new() -> Result<Self> {
        let firecrawl_app = FirecrawlApp::new_selfhosted("http://localhost:13002", Some("fc-test"))
            .context("Firecrawl SDKの初期化に失敗")?;

        Ok(Self { firecrawl_app })
    }

    /// カスタム設定でFirecrawlクライアントを作成
    pub fn new_with_config(base_url: &str, api_key: Option<&str>) -> Result<Self> {
        let firecrawl_app = FirecrawlApp::new_selfhosted(base_url, api_key)
            .context("Firecrawl SDKの初期化に失敗")?;

        Ok(Self { firecrawl_app })
    }
}

#[async_trait]
impl FirecrawlClientProtocol for FirecrawlClient {
    async fn scrape_url(&self, url: &str, _options: Option<()>) -> Result<Document> {
        self.firecrawl_app
            .scrape_url(url, None)
            .await
            .map_err(|e| anyhow::anyhow!("Firecrawl API エラー: {}", e))
    }
}

/// テスト用のモック実装
pub struct FirecrawlClientMock {
    /// モック時に返すマークダウン内容
    pub mock_content: String,
    /// モック時に返すステータス（成功/失敗の制御）
    pub should_succeed: bool,
    /// エラー時に返すメッセージ
    pub error_message: Option<String>,
}

impl FirecrawlClientMock {
    /// 成功レスポンスを返すモッククライアントを作成
    pub fn new_success(mock_content: &str) -> Self {
        Self {
            mock_content: mock_content.to_string(),
            should_succeed: true,
            error_message: None,
        }
    }

    /// エラーレスポンスを返すモッククライアントを作成
    pub fn new_error(error_message: &str) -> Self {
        Self {
            mock_content: String::new(),
            should_succeed: false,
            error_message: Some(error_message.to_string()),
        }
    }
}

#[async_trait]
impl FirecrawlClientProtocol for FirecrawlClientMock {
    async fn scrape_url(&self, _url: &str, _options: Option<()>) -> Result<Document> {
        if self.should_succeed {
            // 成功時のモックレスポンス
            Ok(Document {
                markdown: Some(self.mock_content.clone()),
                // 他のフィールドをデフォルト値で埋める
                ..Default::default()
            })
        } else {
            // エラー時のレスポンス
            let error_msg = self
                .error_message
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("Mock error");
            Err(anyhow::anyhow!("モックエラー: {}", error_msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_client_success() {
        let mock_client = FirecrawlClientMock::new_success("テストマークダウン内容");

        let result = mock_client.scrape_url("https://example.com", None).await;

        assert!(result.is_ok());
        let document = result.unwrap();
        assert_eq!(
            document.markdown,
            Some("テストマークダウン内容".to_string())
        );
    }

    #[tokio::test]
    async fn test_mock_client_error() {
        let mock_client = FirecrawlClientMock::new_error("テストエラー");

        let result = mock_client.scrape_url("https://example.com", None).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("テストエラー"));
    }
}
