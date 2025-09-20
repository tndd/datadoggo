use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Playwright APIからレンダリング済みHTMLを取得するための抽象トレイト
#[async_trait]
pub trait PlaywrightClient {
    /// 指定URLのレンダリング結果を取得する
    ///
    /// # Arguments
    /// * `url` - レンダリング対象のURL
    /// * `options` - レンダリング時のオプション
    async fn fetch_rendered_html(
        &self,
        url: &str,
        options: &PlaywrightRenderOptions,
    ) -> Result<RenderedPage>;
}

/// Playwrightから返却されるレンダリング結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPage {
    /// レンダリングされたHTML全文
    pub html: String,
    /// Playwrightが観測したHTTPステータスコード
    pub status: u16,
    /// リダイレクト後などの最終URL
    pub final_url: String,
}

/// Playwrightレンダリング時のオプション
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaywrightRenderOptions {
    /// Playwrightの`waitUntil`指定（例: `networkidle`）
    pub wait_until: Option<String>,
    /// 指定セレクタが現れるまで待機する場合に利用
    pub wait_for_selector: Option<String>,
    /// Playwrightが待機する最大時間 (ms)
    pub timeout_ms: Option<u64>,
}

impl PlaywrightRenderOptions {
    /// 新しいオプションを生成
    pub fn new() -> Self {
        Self::default()
    }

    /// wait_untilを設定
    pub fn with_wait_until(mut self, value: impl Into<String>) -> Self {
        self.wait_until = Some(value.into());
        self
    }

    /// wait_for_selectorを設定
    pub fn with_wait_for_selector(mut self, selector: impl Into<String>) -> Self {
        self.wait_for_selector = Some(selector.into());
        self
    }

    /// timeout_msを設定
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }
}

/// 実際のPlaywrightサービスと通信するクライアント
pub struct ReqwestPlaywrightClient {
    client: Client,
    base_url: String,
}

impl ReqwestPlaywrightClient {
    /// 既定のエンドポイント(http://localhost:13003)で新規作成
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "http://localhost:13003".to_string(),
        }
    }

    /// 任意のbase_urlでクライアントを生成
    pub fn new_with_base_url(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// テスト等でClientを差し替えたい場合のコンストラクタ
    pub fn new_with_client(base_url: &str, client: Client) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    fn endpoint(&self) -> String {
        format!("{}/render", self.base_url)
    }
}

impl Default for ReqwestPlaywrightClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize)]
struct RenderRequest<'a> {
    url: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    wait_until: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wait_for_selector: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RenderResponseDto {
    html: Option<String>,
    #[serde(default)]
    status: Option<u16>,
    #[serde(default)]
    final_url: Option<String>,
}

#[async_trait]
impl PlaywrightClient for ReqwestPlaywrightClient {
    async fn fetch_rendered_html(
        &self,
        url: &str,
        options: &PlaywrightRenderOptions,
    ) -> Result<RenderedPage> {
        let request = RenderRequest {
            url,
            wait_until: options.wait_until.as_deref(),
            wait_for_selector: options.wait_for_selector.as_deref(),
            timeout_ms: options.timeout_ms,
        };

        let response = self
            .client
            .post(self.endpoint())
            .json(&request)
            .send()
            .await
            .context("Playwright APIへのリクエスト送信に失敗")?;

        let status_code = response.status();
        let dto = response
            .json::<RenderResponseDto>()
            .await
            .context("Playwright APIレスポンスのJSON変換に失敗")?;

        let html = dto
            .html
            .ok_or_else(|| anyhow!("Playwright APIレスポンスにhtmlが含まれていません"))?;

        let status = dto.status.unwrap_or_else(|| status_code.as_u16());
        let final_url = dto.final_url.unwrap_or_else(|| url.to_string());

        Ok(RenderedPage {
            html,
            status,
            final_url,
        })
    }
}

/// テスト用モッククライアント
pub struct MockPlaywrightClient {
    simulate_success: bool,
    mock_html: String,
    mock_status: u16,
    mock_final_url: Option<String>,
    error_message: Option<String>,
}

impl MockPlaywrightClient {
    /// 成功レスポンスを返すモックを作成
    pub fn new_success(mock_html: &str) -> Self {
        Self {
            simulate_success: true,
            mock_html: mock_html.to_string(),
            mock_status: 200,
            mock_final_url: None,
            error_message: None,
        }
    }

    /// エラーレスポンスを返すモックを作成
    pub fn new_error(error_message: &str) -> Self {
        Self {
            simulate_success: false,
            mock_html: String::new(),
            mock_status: 500,
            mock_final_url: None,
            error_message: Some(error_message.to_string()),
        }
    }

    /// 成功時に任意のHTTPステータスを設定
    pub fn with_status(mut self, status: u16) -> Self {
        self.mock_status = status;
        self
    }

    /// 成功時に最終URLを上書き
    pub fn with_final_url(mut self, final_url: &str) -> Self {
        self.mock_final_url = Some(final_url.to_string());
        self
    }
}

#[async_trait]
impl PlaywrightClient for MockPlaywrightClient {
    async fn fetch_rendered_html(
        &self,
        url: &str,
        _options: &PlaywrightRenderOptions,
    ) -> Result<RenderedPage> {
        if !self.simulate_success {
            let error_msg = self
                .error_message
                .as_deref()
                .unwrap_or("Playwright mock error");
            return Err(anyhow!("モックPlaywrightエラー: {}", error_msg));
        }

        Ok(RenderedPage {
            html: self.mock_html.clone(),
            status: self.mock_status,
            final_url: self
                .mock_final_url
                .clone()
                .unwrap_or_else(|| url.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod fetch_rendered_html {
        use super::*;

        /// # テスト目的
        /// - モッククライアントが成功時に設定したHTMLを返すことを確認
        /// # 検証観点
        /// - HTML内容とステータスコード、および最終URLが期待通りであること
        #[tokio::test]
        async fn test_success_returns_mock_html() {
            let client = MockPlaywrightClient::new_success("<html>ok</html>");
            let options = PlaywrightRenderOptions::default();
            let result = client
                .fetch_rendered_html("https://example.com", &options)
                .await
                .unwrap();

            assert_eq!(result.html, "<html>ok</html>");
            assert_eq!(result.status, 200);
            assert_eq!(result.final_url, "https://example.com");
        }

        /// # テスト目的
        /// - エラー設定時に期待通りの失敗を返すことを確認
        /// # 検証観点
        /// - エラーメッセージにモックで指定した内容が含まれること
        #[tokio::test]
        async fn test_error_propagation() {
            let client = MockPlaywrightClient::new_error("render failed");
            let options = PlaywrightRenderOptions::default();
            let result = client
                .fetch_rendered_html("https://example.com", &options)
                .await;

            assert!(result.is_err());
            let err = result.err().unwrap().to_string();
            assert!(err.contains("render failed"));
        }

        /// # テスト目的
        /// - カスタムステータスと最終URLが反映されることを確認
        /// # 検証観点
        /// - with_status/with_final_urlで設定した値が結果に含まれること
        #[tokio::test]
        async fn test_custom_status_and_final_url() {
            let client = MockPlaywrightClient::new_success("<html>ok</html>")
                .with_status(302)
                .with_final_url("https://example.com/redirected");
            let options = PlaywrightRenderOptions::default();

            let result = client
                .fetch_rendered_html("https://example.com", &options)
                .await
                .unwrap();

            assert_eq!(result.status, 302);
            assert_eq!(result.final_url, "https://example.com/redirected");
        }
    }

    #[cfg(feature = "online")]
    mod online {
        use super::*;

        /// # テスト目的
        /// - Playwrightサービス未起動時でも意味のあるエラー文脈が得られることを確認
        /// # 検証観点
        /// - anyhowエラーにコンテキスト文字列が含まれること
        #[tokio::test]
        async fn test_connection_error_context() {
            let client = ReqwestPlaywrightClient::new();
            let options = PlaywrightRenderOptions::default();
            let result = client
                .fetch_rendered_html("https://example.com", &options)
                .await;

            if let Err(err) = result {
                let message = err.to_string();
                assert!(message.contains("Playwright APIへのリクエスト送信に失敗"));
            }
        }
    }
}
