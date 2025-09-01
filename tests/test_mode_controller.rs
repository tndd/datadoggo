//! テストモード制御モジュール
//!
//! このモジュールは、テストの実行モード（モック/オンライン）を
//! 動的に切り替える機能を提供します。
//! 
//! ## 使用方法
//! 
//! ```rust
//! use test_mode_controller::firecrawl;
//! 
//! // モード判定
//! if firecrawl::is_online_mode() {
//!     println!("オンラインテストモード");
//! } else {
//!     println!("モックテストモード");
//! }
//! 
//! // テストセットアップ
//! let setup = firecrawl::setup_test("https://example.com", "モック内容").await;
//! ```

use mock_server::FirecrawlMockServer;

mod mock_server;

/// テストのセットアップ状態を表すenum
pub enum TestSetup {
    /// オンラインモード（実際のFirecrawl APIを使用）
    Online,
    /// モックモード（FirecrawlMockServerを使用）
    Mock(FirecrawlMockServer),
}

/// Firecrawlテスト制御モジュール
pub mod firecrawl {
    use super::*;

    /// オンラインテストモードかどうかを判定する
    /// 
    /// 以下の条件でオンラインモードと判定される：
    /// 1. `online` featureが有効
    /// 2. `TEST_ONLINE` 環境変数が設定されている
    pub fn is_online_mode() -> bool {
        cfg!(feature = "online") || std::env::var("TEST_ONLINE").is_ok()
    }

    /// テスト環境をセットアップする
    /// 
    /// # Arguments
    /// * `url` - テスト対象のURL
    /// * `mock_content` - モックモード時に返される内容
    /// 
    /// # Returns
    /// * `TestSetup::Online` - オンラインモードの場合
    /// * `TestSetup::Mock(server)` - モックモードの場合（モックサーバーを含む）
    pub async fn setup_test(url: &str, mock_content: &str) -> TestSetup {
        if is_online_mode() {
            TestSetup::Online
        } else {
            let mock_server = FirecrawlMockServer::start();
            mock_server.mock_scrape_success(url, mock_content);
            TestSetup::Mock(mock_server)
        }
    }

    /// アサーション用ヘルパー関数
    /// モードに応じて適切なアサーションを実行する
    pub fn assert_article_content(content: &str, expected_mock_content: &str) {
        if is_online_mode() {
            assert!(!content.is_empty(), "オンラインテスト: 記事内容が空です");
            println!("✅ オンライン統合テスト成功: {}文字取得", content.len());
        } else {
            assert!(
                content.contains(expected_mock_content),
                "モックテスト: 期待されるモック内容が含まれていません"
            );
            println!("✅ モックテスト成功: {}文字", content.len());
        }
    }

    /// 統一されたfetch_article関数
    /// テストモードに応じて適切なAPIエンドポイントを使用する
    pub async fn fetch_article_unified(url: &str, mock_server_url: Option<&str>) -> anyhow::Result<datadoggo::domain::article::Article> {
        use datadoggo::domain::article::fetch_article_from_firecrawl_url;
        
        if is_online_mode() {
            // オンラインモード: 実際のFirecrawl API使用
            fetch_article_from_firecrawl_url(url, "http://localhost:13002").await
        } else {
            // モックモード: 提供されたモックサーバーURL使用
            let firecrawl_url = mock_server_url.unwrap_or("http://localhost:8080");
            fetch_article_from_firecrawl_url(url, firecrawl_url).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_detection() {
        // 環境変数をクリアしてテスト
        std::env::remove_var("TEST_ONLINE");
        
        // featureフラグのみでの判定をテスト
        let is_online = firecrawl::is_online_mode();
        
        if cfg!(feature = "online") {
            assert!(is_online, "online featureが有効な場合はオンラインモードのはず");
        } else {
            assert!(!is_online, "online featureが無効な場合はモックモードのはず");
        }
    }

    #[test]
    fn test_env_var_detection() {
        // 環境変数を設定してテスト
        std::env::set_var("TEST_ONLINE", "1");
        
        let is_online = firecrawl::is_online_mode();
        assert!(is_online, "TEST_ONLINE環境変数が設定されている場合はオンラインモード");
        
        // テスト後にクリーンアップ
        std::env::remove_var("TEST_ONLINE");
    }

    #[tokio::test]
    async fn test_mock_setup() {
        // 環境変数をクリアしてモックモードを強制
        std::env::remove_var("TEST_ONLINE");
        
        let setup = firecrawl::setup_test("https://test.com", "テスト内容").await;
        
        match setup {
            TestSetup::Online => {
                if cfg!(feature = "online") {
                    println!("✅ online featureが有効なためオンラインモード");
                } else {
                    panic!("モックモードが期待されましたがオンラインモードになりました");
                }
            }
            TestSetup::Mock(_server) => {
                println!("✅ モックサーバーが正常にセットアップされました");
            }
        }
    }
}