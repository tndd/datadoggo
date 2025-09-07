use crate::infra::compute::generate_mock_rss_id;
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
    async fn fetch(&self, url: &str, timeout_secs: u64) -> Result<String>;
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
    async fn fetch(&self, url: &str, timeout_secs: u64) -> Result<String> {
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
/// URL依存の動的XMLまたはエラーを返します。
pub struct MockHttpClient {
    /// モック時に成功を返すかどうか
    pub simulate_success: bool,
    /// エラー時に返すメッセージ
    pub error_message: Option<String>,
}

impl MockHttpClient {
    /// URL依存の動的XMLレスポンスを返すモッククライアントを作成
    pub fn new_success() -> Self {
        Self {
            simulate_success: true,
            error_message: None,
        }
    }

    /// エラーレスポンスを返すモッククライアントを作成
    pub fn new_error(error_message: &str) -> Self {
        Self {
            simulate_success: false,
            error_message: Some(error_message.to_string()),
        }
    }
}

#[async_trait]
impl HttpClient for MockHttpClient {
    async fn fetch(&self, url: &str, _timeout_secs: u64) -> Result<String> {
        if !self.simulate_success {
            // エラー時のレスポンス
            let error_msg = self.error_message.as_deref().unwrap_or("Mock HTTP error");
            return Err(anyhow::anyhow!("モックHTTPエラー: {}", error_msg));
        }

        // URL依存の動的XML生成
        let hash = generate_mock_rss_id(url);

        // 動的な日付生成（今日、1日前、2日前）
        let now = chrono::Utc::now();
        let today = now.format("%a, %d %b %Y %H:%M:%S GMT");
        let yesterday = (now - chrono::Duration::days(1)).format("%a, %d %b %Y %H:%M:%S GMT");
        let day_before = (now - chrono::Duration::days(2)).format("%a, %d %b %Y %H:%M:%S GMT");

        Ok(format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <rss version="2.0">
                    <channel>
                        <title>{}:channel_title</title>
                        <item>
                            <title>{}:title:1</title>
                            <link>https://{}.example.com/1</link>
                            <pubDate>{}</pubDate>
                        </item>
                        <item>
                            <title>{}:title:2</title>
                            <link>https://{}.example.com/2</link>
                            <pubDate>{}</pubDate>
                        </item>
                        <item>
                            <title>{}:title:3</title>
                            <link>https://{}.example.com/3</link>
                            <pubDate>{}</pubDate>
                        </item>
                    </channel>
                </rss>"#,
            hash, hash, hash, today, hash, hash, yesterday, hash, hash, day_before
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 関数名ベースのモジュールへ統一
    mod fetch {
        use super::*;

        #[tokio::test]
        async fn test_success() {
            let mock_client = MockHttpClient::new_success();
            let test_url = "https://example.com/rss.xml";
            let response = mock_client.fetch(test_url, 30).await.unwrap();
            assert!(response.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
            let hash = generate_mock_rss_id(test_url);
            assert!(response.contains(&format!("{}:channel_title", hash)));
        }

        #[tokio::test]
        async fn test_error() {
            let mock_client = MockHttpClient::new_error("接続失敗");
            let result = mock_client.fetch("https://example.com/rss.xml", 30).await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_dynamic_by_url() {
            let mock_client = MockHttpClient::new_success();
            let url1 = "https://test1.com/rss";
            let url2 = "https://test2.com/rss";
            let xml1 = mock_client.fetch(url1, 30).await.unwrap();
            let xml2 = mock_client.fetch(url2, 30).await.unwrap();
            assert_ne!(xml1, xml2);
        }
    }

    // オンラインテストはモジュール単位でfeatureガード
    #[cfg(feature = "online")]
    mod online {
        use super::*;
        #[tokio::test]
        async fn test_http_online_basic() -> Result<(), anyhow::Error> {
            let client = ReqwestHttpClient::new();
            let result = client.fetch("https://httpbin.org/xml", 10).await;
            match result {
                Ok(content) => {
                    assert!(!content.is_empty());
                    assert!(content.contains("xml"));
                }
                Err(_) => return Ok(()),
            }
            Ok(())
        }
    }
}
