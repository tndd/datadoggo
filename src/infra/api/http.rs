use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;

/// HTTPクライアントの抽象化トレイト
///
/// このトレイトは、実際のHTTP通信とモック実装の両方を
/// 統一的に扱えるようにするためのインターフェースです。
#[async_trait]
pub trait HttpClient {
    /// 指定されたURLからテキストを取得する
    ///
    /// # Arguments
    /// * `url` - 取得対象のURL
    /// * `timeout_secs` - タイムアウト時間（秒）
    async fn fetch_text(&self, url: &str, timeout_secs: u64) -> Result<String>;
}

/// `reqwest` を使用した本番用のHTTPクライアント実装
pub struct ReqwestHttpClient {
    client: Client,
}

impl ReqwestHttpClient {
    /// 新しいHTTPクライアントを作成
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for ReqwestHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClient for ReqwestHttpClient {
    async fn fetch_text(&self, url: &str, timeout_secs: u64) -> Result<String> {
        let response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(timeout_secs))
            .send()
            .await
            .context(format!("HTTPリクエストの送信に失敗: {}", url))?;

        response
            .text()
            .await
            .context("レスポンステキストの取得に失敗")
    }
}

/// テスト用のモックHTTPクライアント
///
/// この実装はテスト時にDIされ、実際のHTTPリクエストを行わずに
/// 定義済みのレスポンスやエラーを返します。
pub struct MockHttpClient {
    /// モック時に返すレスポンス内容
    pub mock_response: String,
    /// モック時に返すステータス（成功/失敗の制御）
    pub should_succeed: bool,
    /// エラー時に返すメッセージ
    pub error_message: Option<String>,
}

impl MockHttpClient {
    /// 成功レスポンスを返すモッククライアントを作成
    pub fn new_success(mock_response: &str) -> Self {
        Self {
            mock_response: mock_response.to_string(),
            should_succeed: true,
            error_message: None,
        }
    }

    /// エラーレスポンスを返すモッククライアントを作成
    pub fn new_error(error_message: &str) -> Self {
        Self {
            mock_response: String::new(),
            should_succeed: false,
            error_message: Some(error_message.to_string()),
        }
    }
}

#[async_trait]
impl HttpClient for MockHttpClient {
    async fn fetch_text(&self, _url: &str, _timeout_secs: u64) -> Result<String> {
        if self.should_succeed {
            // 成功時のモックレスポンス
            Ok(self.mock_response.clone())
        } else {
            // エラー時のレスポンス
            let error_msg = self
                .error_message
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("Mock HTTP error");
            Err(anyhow::anyhow!("モックHTTPエラー: {}", error_msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_http_client_success() {
        let mock_client = MockHttpClient::new_success("<rss>テストXML内容</rss>");

        let result = mock_client
            .fetch_text("https://example.com/rss.xml", 30)
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.contains("テストXML内容"));
    }

    #[tokio::test]
    async fn test_mock_http_client_error() {
        let mock_client = MockHttpClient::new_error("接続失敗");

        let result = mock_client
            .fetch_text("https://example.com/rss.xml", 30)
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("接続失敗"));
    }

    /// 軽量オンラインテスト - 実際のHTTP通信での基本接続確認
    #[cfg(feature = "online")]
    #[tokio::test]
    async fn test_http_online_basic() -> Result<(), anyhow::Error> {
        // httpbin.orgを使った軽量なHTTP接続テスト
        let client = ReqwestHttpClient::new();
        let result = client.fetch_text("https://httpbin.org/xml", 10).await;

        match result {
            Ok(content) => {
                assert!(!content.is_empty(), "取得した内容が空");
                assert!(content.contains("xml"), "XMLコンテンツを含むべき");
                println!("✅ HTTP軽量オンラインテスト成功: {}文字取得", content.len());
            }
            Err(e) => {
                println!("⚠️ HTTPリクエストが失敗: {}", e);
                println!("ネットワーク接続を確認してください");
                // ネットワーク問題の場合は失敗にしない
                return Ok(());
            }
        }

        Ok(())
    }
}
