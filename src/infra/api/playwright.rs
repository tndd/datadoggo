use anyhow::{anyhow, Context, Result};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

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

pub async fn fetch_rendered_html(
    url: &str,
    options: &PlaywrightRenderOptions,
) -> Result<RenderedPage> {
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .expect("Playwright HTTPクライアントの初期化に失敗");

    let endpoint = "http://localhost:13003/render".to_string();

    let request = RenderRequest {
        url,
        wait_until: options.wait_until.as_deref(),
        wait_for_selector: options.wait_for_selector.as_deref(),
        timeout_ms: options.timeout_ms,
    };

    let response = client
        .post(&endpoint)
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
        .context("Playwright APIレスポンスにhtmlが含まれていません")?;

    let status = dto.status.unwrap_or_else(|| status_code.as_u16());

    let final_url = dto.final_url.unwrap_or_else(|| url.to_string());

    Ok(RenderedPage {
        html,
        status,
        final_url,
    })
}

pub async fn mock_fetch_rendered_html(
    url: &str,
    _options: &PlaywrightRenderOptions,
    mock_options: MockOptions,
) -> Result<RenderedPage> {
    if let Some(delay) = mock_options.delay {
        tokio::time::sleep(delay).await;
    }

    if !mock_options.simulate_success {
        let error_msg = mock_options
            .error_message
            .unwrap_or("Playwright mock error".to_string());
        return Err(anyhow!(error_msg));
    }

    Ok(RenderedPage {
        html: mock_options.mock_html,
        status: mock_options.mock_status,
        final_url: mock_options
            .mock_final_url
            .unwrap_or_else(|| url.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    mod fetch_rendered_html {
        use super::*;
        use tokio::time::Instant;

        /// # テスト目的
        /// - モック関数が成功時に指定したHTMLを返すことを確認
        /// # 検証観点
        /// - HTML内容とステータスコード、および最終URLが期待通りであること
        #[tokio::test]
        async fn test_success_returns_mock_html() {
            let options = PlaywrightRenderOptions::default();
            let result = mock_fetch_rendered_html(
                "https://example.com",
                &options,
                MockOptions {
                    simulate_success: true,
                    mock_html: "<html>ok</html>".to_string(),
                    mock_status: 200,
                    mock_final_url: None,
                    error_message: None,
                    delay: None,
                },
            )
            .await
            .unwrap();

            assert_eq!(result.html, "<html>ok</html>");
            assert_eq!(result.status, 200);
            assert_eq!(result.final_url, "https://example.com");
        }

        /// # テスト目的
        /// - モック関数がエラー設定時に期待通りの失敗を返すことを確認
        /// # 検証観点
        /// - エラーメッセージに指定した内容が含まれること
        #[tokio::test]
        async fn test_error_propagation() {
            let options = PlaywrightRenderOptions::default();
            let result = mock_fetch_rendered_html(
                "https://example.com",
                &options,
                MockOptions {
                    simulate_success: false,
                    mock_html: "".to_string(),
                    mock_status: 500,
                    mock_final_url: None,
                    error_message: Some("render failed".to_string()),
                    delay: None,
                },
            )
            .await;

            assert!(result.is_err());
            let err = result.err().unwrap().to_string();
            assert!(err.contains("render failed"));
        }

        /// # テスト目的
        /// - モック関数がカスタムステータスと最終URLを反映することを確認
        /// # 検証観点
        /// - 指定したステータスとURLが結果に含まれること
        #[tokio::test]
        async fn test_custom_status_and_final_url() {
            let options = PlaywrightRenderOptions::default();
            let result = mock_fetch_rendered_html(
                "https://example.com",
                &options,
                MockOptions {
                    simulate_success: true,
                    mock_html: "<html>ok</html>".to_string(),
                    mock_status: 302,
                    mock_final_url: Some("https://example.com/redirected".to_string()),
                    error_message: None,
                    delay: None,
                },
            )
            .await
            .unwrap();

            assert_eq!(result.status, 302);
            assert_eq!(result.final_url, "https://example.com/redirected");
        }

        /// # テスト目的
        /// - モック関数がリダイレクトをシミュレートできることを確認
        /// # 検証観点
        /// - 指定した302ステータスと最終URLが反映されること
        #[tokio::test]
        async fn test_redirect_accessor_sets_status_and_url() {
            let options = PlaywrightRenderOptions::default();
            let result = mock_fetch_rendered_html(
                "https://example.com/original",
                &options,
                MockOptions {
                    simulate_success: true,
                    mock_html: "<html>ok</html>".to_string(),
                    mock_status: 302,
                    mock_final_url: Some("https://example.com/final".to_string()),
                    error_message: None,
                    delay: None,
                },
            )
            .await
            .unwrap();

            assert_eq!(result.status, 302);
            assert_eq!(result.final_url, "https://example.com/final");
        }

        /// # テスト目的
        /// - モック関数が指定した遅延を待機することを確認
        /// # 検証観点
        /// - 処理完了までに指定ミリ秒以上かかること
        #[tokio::test]
        async fn test_delay_simulation_waits_for_configured_duration() {
            let options = PlaywrightRenderOptions::default();

            let start = Instant::now();
            let _ = mock_fetch_rendered_html(
                "https://example.com",
                &options,
                MockOptions {
                    simulate_success: true,
                    mock_html: "<html>ok</html>".to_string(),
                    mock_status: 200,
                    mock_final_url: None,
                    error_message: None,
                    delay: Some(Duration::from_millis(20)),
                },
            )
            .await
            .unwrap();
            let elapsed = start.elapsed();

            assert!(elapsed >= Duration::from_millis(20));
        }
    }

    #[cfg(feature = "online")]
    mod online {
        use super::*;

        /// # テスト目的
        /// - 実際のfetch関数が接続エラー時に適切なコンテキストを返すことを確認
        /// # 検証観点
        /// - エラーメッセージにリクエスト失敗のコンテキストが含まれること
        #[tokio::test]
        async fn test_connection_error_context() {
            let options = PlaywrightRenderOptions::default();
            let result = fetch_rendered_html("https://example.com", &options).await;

            if let Err(err) = result {
                let message = err.to_string();
                assert!(message.contains("Playwright APIへのリクエスト送信に失敗"));
            }
        }
    }
}

/// モック関数のオプションをまとめた構造体
#[derive(Debug, Default)]
pub struct MockOptions {
    /// 成功をシミュレートするかどうか
    pub simulate_success: bool,
    /// モックHTML内容
    pub mock_html: String,
    /// モックHTTPステータス
    pub mock_status: u16,
    /// モック最終URL (オプション)
    pub mock_final_url: Option<String>,
    /// エラーメッセージ (オプション)
    pub error_message: Option<String>,
    /// 遅延時間 (オプション)
    pub delay: Option<Duration>,
}
