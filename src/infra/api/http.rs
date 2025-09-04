use crate::infra::compute::calc_hash;
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
    /// モック時に返すステータス（成功/失敗の制御）
    pub should_succeed: bool,
    /// エラー時に返すメッセージ
    pub error_message: Option<String>,
}

impl MockHttpClient {
    /// URL依存の動的XMLレスポンスを返すモッククライアントを作成
    pub fn new_success() -> Self {
        Self {
            should_succeed: true,
            error_message: None,
        }
    }

    /// エラーレスポンスを返すモッククライアントを作成
    pub fn new_error(error_message: &str) -> Self {
        Self {
            should_succeed: false,
            error_message: Some(error_message.to_string()),
        }
    }
}

#[async_trait]
impl HttpClient for MockHttpClient {
    async fn fetch(&self, url: &str, _timeout_secs: u64) -> Result<String> {
        if !self.should_succeed {
            // エラー時のレスポンス
            let error_msg = self
                .error_message
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("Mock HTTP error");
            return Err(anyhow::anyhow!("モックHTTPエラー: {}", error_msg));
        }

        // URL依存の動的XML生成
        let hash = calc_hash(url, 6);

        Ok(format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
                            <rss version="2.0">
                                <channel>
                                    <title>{}:channel_title</title>
                                    <item>
                                        <title>{}:title:1</title>
                                        <link>https://{}.example.com/1</link>
                                        <pubDate>Wed, 01 Jan 2025 12:00:00 GMT</pubDate>
                                    </item>
                                    <item>
                                        <title>{}:title:2</title>
                                        <link>https://{}.example.com/2</link>
                                        <pubDate>Thu, 02 Jan 2025 12:00:00 GMT</pubDate>
                                    </item>
                                    <item>
                                        <title>{}:title:3</title>
                                        <link>https://{}.example.com/3</link>
                                        <pubDate>Fri, 03 Jan 2025 12:00:00 GMT</pubDate>
                                    </item>
                                </channel>
                            </rss>"#,
            hash, hash, hash, hash, hash, hash, hash
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_http_client_success() {
        let mock_client = MockHttpClient::new_success();
        let test_url = "https://example.com/rss.xml";

        let result = mock_client.fetch(test_url, 30).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        // 動的XMLの基本構造を確認
        assert!(response.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(response.contains("<rss version=\"2.0\">"));
        assert!(response.contains("<channel>"));
        assert!(response.contains("<item>"));
        assert!(response.contains("<title>"));
        assert!(response.contains("<link>"));
        assert!(response.contains("<pubDate>"));

        // URLハッシュが含まれていることを確認
        let hash = calc_hash(test_url, 6);
        assert!(response.contains(&format!("{}:channel_title", hash)));
        assert!(response.contains(&format!("{}:title:1", hash)));
    }

    #[tokio::test]
    async fn test_mock_http_client_error() {
        let mock_client = MockHttpClient::new_error("接続失敗");

        let result = mock_client.fetch("https://example.com/rss.xml", 30).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("接続失敗"));
    }

    #[tokio::test]
    async fn test_mock_http_client_dynamic() {
        let mock_client = MockHttpClient::new_success();

        // 異なるURLで異なるXMLが生成されることを確認
        let url1 = "https://test1.com/rss";
        let url2 = "https://test2.com/rss";
        let result1 = mock_client.fetch(url1, 30).await;
        let result2 = mock_client.fetch(url2, 30).await;

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let xml1 = result1.unwrap();
        let xml2 = result2.unwrap();

        // 各URLのハッシュを計算して期待値を生成
        let hash1 = calc_hash(url1, 6);
        let hash2 = calc_hash(url2, 6);

        // ハッシュが確実に異なることを確認
        assert_ne!(hash1, hash2, "異なるURLから同じハッシュが生成されました");

        // ハッシュを含む特定の文字列でcontains検証
        // xml1
        assert!(xml1.contains(&format!("{}:channel_title", hash1)));
        assert!(xml1.contains(&format!("{}:title:1", hash1)));
        assert!(xml1.contains(&format!("{}:title:2", hash1)));
        assert!(xml1.contains(&format!("{}:title:3", hash1)));
        assert!(xml1.contains(&format!("https://{}.example.com/1", hash1)));
        assert!(xml1.contains(&format!("https://{}.example.com/2", hash1)));
        assert!(xml1.contains(&format!("https://{}.example.com/3", hash1)));
        // xml2
        assert!(xml2.contains(&format!("{}:channel_title", hash2)));
        assert!(xml2.contains(&format!("{}:title:1", hash2)));
        assert!(xml2.contains(&format!("{}:title:2", hash2)));
        assert!(xml2.contains(&format!("{}:title:3", hash2)));
        assert!(xml2.contains(&format!("https://{}.example.com/1", hash2)));
        assert!(xml2.contains(&format!("https://{}.example.com/2", hash2)));
        assert!(xml2.contains(&format!("https://{}.example.com/3", hash2)));

        // 異なるURLから生成されたXMLは異なる内容であることを確認
        assert_ne!(xml1, xml2, "異なるURLから同じXMLが生成されました");

        println!("✅ 動的XML生成テスト成功");
        println!("hash1: {} / hash2: {}", hash1, hash2);
        println!("XML1の長さ: {}文字", xml1.len());
        println!("XML2の長さ: {}文字", xml2.len());
    }

    /// 軽量オンラインテスト - 実際のHTTP通信での基本接続確認
    #[cfg(feature = "online")]
    #[tokio::test]
    async fn test_http_online_basic() -> Result<(), anyhow::Error> {
        // httpbin.orgを使った軽量なHTTP接続テスト
        let client = ReqwestHttpClient::new();
        let result = client.fetch("https://httpbin.org/xml", 10).await;

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
