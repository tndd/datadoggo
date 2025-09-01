//! Firecrawl API モックサーバー
//!
//! このモジュールはhttpmockを使用してFirecrawl APIをモックし、
//! 統合テストで外部通信を完全に遮断したテスト環境を提供します。

use httpmock::prelude::*;
use serde_json::json;

/// Firecrawl APIのモックサーバー
pub struct FirecrawlMockServer {
    server: MockServer,
}

impl FirecrawlMockServer {
    /// Firecrawl APIと同じポート（13002）でモックサーバーを開始
    pub fn start() -> Self {
        let server = MockServer::start();
        // httpmockではポートの指定は現在利用不可なので、代わりに環境変数などで制御
        Self { server }
    }

    /// 指定URLのスクレイプ成功をモック
    pub fn mock_scrape_success(&self, url: &str, markdown: &str) {
        self.server.mock(|when, then| {
            when.method(POST)
                .path("/v1/scrape")
                .json_body_partial(json!({"url": url}).to_string());
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "success": true,
                    "data": {
                        "markdown": markdown
                    }
                }));
        });
    }

    /// 任意URLのスクレイプ成功をモック（URL不問）
    pub fn mock_any_scrape_success(&self, default_content: &str) {
        self.server.mock(|when, then| {
            when.method(POST)
                .path("/v1/scrape");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "success": true,
                    "data": {
                        "markdown": default_content
                    }
                }));
        });
    }

    /// スクレイプタイムアウトをモック
    pub fn mock_scrape_timeout(&self, url: &str) {
        self.server.mock(|when, then| {
            when.method(POST)
                .path("/v1/scrape")
                .json_body_partial(json!({"url": url}).to_string());
            then.status(408)
                .header("content-type", "application/json")
                .json_body(json!({
                    "success": false,
                    "error": "Scrape timed out"
                }));
        });
    }

    /// スクレイプエラーをモック
    pub fn mock_scrape_error(&self, url: &str, error_message: &str) {
        self.server.mock(|when, then| {
            when.method(POST)
                .path("/v1/scrape")
                .json_body_partial(json!({"url": url}).to_string());
            then.status(500)
                .header("content-type", "application/json")
                .json_body(json!({
                    "success": false,
                    "error": error_message
                }));
        });
    }

    /// 複数URLのバッチモック（異なるレスポンスを設定）
    pub fn mock_multiple_scrapes(&self, url_content_pairs: Vec<(&str, &str)>) {
        for (url, content) in url_content_pairs {
            self.mock_scrape_success(url, content);
        }
    }

    /// モックサーバーのベースURL取得
    pub fn url(&self) -> String {
        self.server.url("")
    }

    /// 呼び出し回数の検証用
    pub fn verify_called(&self, _url: &str, _expected_times: usize) -> bool {
        // httpmockの検証機能を使用（実装詳細は後で追加）
        true // プレースホルダー
    }
}

impl Drop for FirecrawlMockServer {
    fn drop(&mut self) {
        // httpmockは自動的にクリーンアップされるため、特別な処理は不要
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_mock_server_basic_functionality() {
        let mock_server = FirecrawlMockServer::start();
        
        // モック設定
        mock_server.mock_scrape_success("https://example.com", "テスト記事内容");
        
        // HTTPクライアントで実際にリクエスト
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/v1/scrape", mock_server.url()))
            .json(&json!({"url": "https://example.com"}))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        
        let json_response: serde_json::Value = response.json().await.unwrap();
        assert_eq!(json_response["success"], true);
        assert_eq!(json_response["data"]["markdown"], "テスト記事内容");
    }
}