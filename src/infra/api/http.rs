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
/// 定義済みのレスポンスやエラーを返します。
pub struct MockHttpClient {
    /// モック時に返すレスポンス内容（Noneの場合は動的生成）
    pub mock_response: Option<String>,
    /// モック時に返すステータス（成功/失敗の制御）
    pub should_succeed: bool,
    /// エラー時に返すメッセージ
    pub error_message: Option<String>,
}

impl MockHttpClient {
    /// 成功レスポンスを返すモッククライアントを作成
    pub fn new_success(mock_response: &str) -> Self {
        Self {
            mock_response: Some(mock_response.to_string()),
            should_succeed: true,
            error_message: None,
        }
    }

    /// エラーレスポンスを返すモッククライアントを作成
    pub fn new_error(error_message: &str) -> Self {
        Self {
            mock_response: Some(String::new()),
            should_succeed: false,
            error_message: Some(error_message.to_string()),
        }
    }

    /// URL依存の動的XMLレスポンスを返すモッククライアントを作成
    pub fn new_dynamic() -> Self {
        Self {
            mock_response: None, // 動的生成のためNone
            should_succeed: true,
            error_message: None,
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

        match &self.mock_response {
            Some(response) => {
                // 固定レスポンスを返す（従来の動作）
                Ok(response.clone())
            }
            None => {
                // URL依存の動的XML生成
                let hash = format!(
                    "{:x}",
                    url.chars().fold(0u64, |acc, c| acc.wrapping_add(c as u64))
                );
                let hash = &hash[..6.min(hash.len())]; // 6文字に制限

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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_http_client_success() {
        let mock_client = MockHttpClient::new_success("<rss>テストXML内容</rss>");

        let result = mock_client.fetch("https://example.com/rss.xml", 30).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.contains("テストXML内容"));
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
        let mock_client = MockHttpClient::new_dynamic();

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
        let hash1 = format!("{:x}", url1.chars().fold(0u64, |acc, c| acc.wrapping_add(c as u64)));
        let hash1 = &hash1[..6.min(hash1.len())];
        let hash2 = format!("{:x}", url2.chars().fold(0u64, |acc, c| acc.wrapping_add(c as u64)));
        let hash2 = &hash2[..6.min(hash2.len())];

        // ハッシュが確実に異なることを確認
        assert_ne!(hash1, hash2, "異なるURLから同じハッシュが生成されました");

        // 両方とも有効なRSS XMLであることを確認
        assert!(xml1.contains("<?xml version=\"1.0\""));
        assert!(xml1.contains("<rss version=\"2.0\">"));
        assert!(xml1.contains(":channel_title"));
        assert!(xml1.contains(":title:1"));
        assert!(xml1.contains(":title:2"));
        assert!(xml1.contains(":title:3"));
        assert!(xml1.contains(".example.com/1"));

        assert!(xml2.contains("<?xml version=\"1.0\""));
        assert!(xml2.contains("<rss version=\"2.0\">"));
        assert!(xml2.contains(":channel_title"));
        assert!(xml2.contains(":title:1"));
        assert!(xml2.contains(":title:2"));
        assert!(xml2.contains(":title:3"));
        assert!(xml2.contains(".example.com/1"));

        // XML1にはhash1が含まれ、hash2は含まれないことを確認
        assert!(xml1.contains(hash1), "XML1にhash1({})が含まれていません", hash1);
        assert!(!xml1.contains(hash2), "XML1にhash2({})が含まれています", hash2);

        // XML2にはhash2が含まれ、hash1は含まれないことを確認
        assert!(xml2.contains(hash2), "XML2にhash2({})が含まれていません", hash2);
        assert!(!xml2.contains(hash1), "XML2にhash1({})が含まれています", hash1);

        // 各XMLの特定パターンが異なることを確認
        assert!(xml1.contains(&format!("https://{}.example.com/1", hash1)));
        assert!(xml1.contains(&format!("{}:title:1", hash1)));
        assert!(xml1.contains(&format!("{}:channel_title", hash1)));
        
        assert!(xml2.contains(&format!("https://{}.example.com/1", hash2)));
        assert!(xml2.contains(&format!("{}:title:1", hash2)));
        assert!(xml2.contains(&format!("{}:channel_title", hash2)));

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
