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
//! // クライアント作成
//! let client = firecrawl::create_client("モック内容");
//! ```

use datadoggo::domain::article::fetch_article_with_mock;

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

    /// 統一されたfetch_article関数
    /// テストモードに応じて適切な実装を使用する
    pub async fn fetch_article_unified(
        url: &str,
        mock_content: &str,
    ) -> anyhow::Result<datadoggo::domain::article::Article> {
        if is_online_mode() {
            // オンラインモード: 実際のAPI呼び出し
            fetch_article_with_mock(url, None).await
        } else {
            // モックモード: モック内容を使用
            fetch_article_with_mock(url, Some(mock_content)).await
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
            assert!(
                is_online,
                "online featureが有効な場合はオンラインモードのはず"
            );
        } else {
            assert!(!is_online, "online featureが無効な場合はモックモードのはず");
        }
    }

    #[test]
    fn test_env_var_detection() {
        // 環境変数を設定してテスト
        std::env::set_var("TEST_ONLINE", "1");

        let is_online = firecrawl::is_online_mode();
        assert!(
            is_online,
            "TEST_ONLINE環境変数が設定されている場合はオンラインモード"
        );

        // テスト後にクリーンアップ
        std::env::remove_var("TEST_ONLINE");
    }

    #[tokio::test]
    async fn test_unified_mode() {
        // 環境変数をクリアしてモックモードを強制
        std::env::remove_var("TEST_ONLINE");

        let result = firecrawl::fetch_article_unified("https://test.com", "テスト内容").await;

        if cfg!(feature = "online") {
            // online featureが有効な場合は実際のAPI呼び出し
            println!("✅ online featureが有効 - 実際のAPI呼び出しまたはフォールバック");
        } else {
            // online featureが無効な場合はモック
            assert!(result.is_ok(), "統一記事取得が失敗");
            let article = result.unwrap();
            assert!(article.content.contains("テスト内容"));
            println!("✅ モック統一テストが正常に完了しました");
        }
    }

    #[tokio::test]
    async fn test_unified_fetch_article() {
        let result = firecrawl::fetch_article_unified("https://test.com", "統一テスト内容").await;

        assert!(result.is_ok(), "統一記事取得が失敗");
        let article = result.unwrap();

        firecrawl::assert_article_content(&article.content, "統一テスト内容");
        println!("✅ 統一記事取得テスト成功");
    }
}
