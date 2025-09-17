use crate::{
    core::rss::{search_rss_links, RssLinkQuery},
    infra::api::{firecrawl::FirecrawlClient, http::HttpClient},
};
use anyhow::{Context, Result};
use sqlx::PgPool;

pub mod article;
pub mod rss;

use article::collect_backlog_articles_with_firecrawl;
use rss::collect_article_links_with_rss_links;

/// ニュースワークフローのメイン実行関数（依存性を注入）
///
/// 1. rss/link.ymlからフィード設定を読み込み
/// 2. 各RSSフィードからリンクを取得してDBに保存
/// 3. 未処理のリンクから記事内容を取得してDBに保存
pub async fn workflow_news<H: HttpClient, F: FirecrawlClient>(
    http_client: &H,
    firecrawl_client: &F,
    pool: &PgPool,
    query: Option<&RssLinkQuery>,
) -> Result<()> {
    match &query {
        Some(rss_query) => {
            println!("=== ニュースワークフロー開始（クエリ指定）===");
            println!("対象クエリ: {:?}", rss_query);
        }
        None => {
            println!("=== ニュースワークフロー開始 ===");
        }
    }

    let rss_links = search_rss_links(query).context("フィード設定の読み込みに失敗")?;

    if rss_links.is_empty() {
        println!("対象のフィードが見つかりませんでした");
        return Ok(());
    }
    println!("対象フィード数: {}件", rss_links.len());

    // 段階1: RSSフィードからリンクを取得
    collect_article_links_with_rss_links(http_client, &rss_links, pool).await?;
    // 段階2: 未処理のリンクから記事内容を取得
    collect_backlog_articles_with_firecrawl(firecrawl_client, pool).await?;

    println!("=== ニュースワークフロー完了 ===");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::rss::{search_rss_links, RssLinkQuery};
    use crate::infra::api::{firecrawl::MockFirecrawlClient, http::MockHttpClient};
    use crate::infra::storage::db::setup_test_db;

    // 関数名ベースのモジュールへ統一
    mod workflow_news {
        use super::*;

        /// 実際のrss/link.ymlを使用して、workflow_newsが正しく動作することをテスト
        #[tokio::test]
        async fn test_basic() -> Result<(), anyhow::Error> {
            let (_container, pool) = setup_test_db().await;
            // 実際のrss/link.ymlからBBCグループのフィード数を取得
            let bbc_query = Some(RssLinkQuery::from_group("bbc"));
            let bbc_rss_links = search_rss_links(bbc_query.as_ref())?;
            let expected_bbc_links_count = bbc_rss_links.len();

            assert!(
                expected_bbc_links_count > 0,
                "BBCグループのフィードが見つかりません。rss/link.ymlを確認してください"
            );

            println!("BBCフィード数: {}件", expected_bbc_links_count);

            // モッククライアントの準備
            let mock_http_client = MockHttpClient::new_success();
            let mock_firecrawl_client =
                MockFirecrawlClient::new_success("BBC統合テスト記事の内容です");

            // 初期状態の確認
            let initial_rss_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            let initial_article_count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;

            assert_eq!(
                initial_rss_count.unwrap_or(0),
                0,
                "初期状態でarticle_linksが空ではありません"
            );
            assert_eq!(
                initial_article_count.unwrap_or(0),
                0,
                "初期状態でarticlesが空ではありません"
            );

            // workflow_newsを実行（実際のrss/link.ymlを使用してBBCグループを指定）
            let result = workflow_news(
                &mock_http_client,
                &mock_firecrawl_client,
                &pool,
                bbc_query.as_ref(),
            )
            .await;

            assert!(
                result.is_ok(),
                "BBC統合ワークフロー実行が失敗しました: {:?}",
                result.err()
            );

            // 結果確認: RSS収集段階
            let final_rss_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            let expected_rss_count = expected_bbc_links_count * 3; // 各フィードから3記事生成
            assert_eq!(
                final_rss_count.unwrap_or(0),
                expected_rss_count as i64,
                "RSS収集段階で期待される数のリンクが保存されませんでした"
            );

            // 結果確認: 記事取得段階
            let final_article_count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                final_article_count.unwrap_or(0),
                expected_rss_count as i64, // RSS収集ですべてが記事として取得される
                "記事取得段階で期待される数の記事が保存されませんでした"
            );

            // 記事内容の確認（成功記事）
            let success_articles =
                sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 200")
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(
                success_articles.unwrap_or(0),
                expected_rss_count as i64,
                "すべての記事が成功ステータスで保存されるべきです"
            );

            // 特定の記事内容確認（最初の記事をチェック）
            let first_article_content: Option<String> =
                sqlx::query_scalar!("SELECT content FROM articles LIMIT 1")
                    .fetch_optional(&pool)
                    .await?;

            assert!(first_article_content.is_some(), "記事内容が見つかりません");
            assert!(
                first_article_content
                    .as_ref()
                    .is_some_and(|content| content.contains("BBC統合テスト記事の内容です")),
                "記事内容が期待されるモック内容を含んでいません: {:?}",
                first_article_content
            );

            println!("✅ workflow_news BBC統合テスト完了");
            println!("  BBCフィード数: {}", expected_bbc_links_count);
            println!("  保存されたRSSリンク数: {}", final_rss_count.unwrap_or(0));
            println!("  保存された記事数: {}", final_article_count.unwrap_or(0));
            println!("  実際のrss/link.ymlからの読み込み: 成功");

            Ok(())
        }

        #[tokio::test]
        async fn test_http_error() -> Result<(), anyhow::Error> {
            let (_container, pool) = setup_test_db().await;
            // エラーシナリオ: HTTP取得エラー（実際のrss/link.yml使用）
            let error_http_client = MockHttpClient::new_error("RSS取得接続エラー");
            let success_firecrawl_client = MockFirecrawlClient::new_success("記事内容");

            let bbc_query = Some(RssLinkQuery::from_group("bbc"));
            let result_http_error = workflow_news(
                &error_http_client,
                &success_firecrawl_client,
                &pool,
                bbc_query.as_ref(),
            )
            .await;

            // ワークフロー全体は成功する（エラーハンドリングにより継続処理）
            assert!(
                result_http_error.is_ok(),
                "HTTP取得エラー時もワークフローは成功するべきです"
            );

            // RSS取得エラーのため、article_linksテーブルにデータなし
            let rss_count_after_http_error =
                sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(
                rss_count_after_http_error.unwrap_or(0),
                0,
                "HTTP取得エラー時はRSSリンクが保存されないべきです"
            );

            // 記事取得処理も実行されるが、未処理リンクがないため記事も0件
            let article_count_after_http_error =
                sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(
                article_count_after_http_error.unwrap_or(0),
                0,
                "HTTP取得エラー時は記事も保存されないべきです"
            );

            println!("✅ workflow_news エラーハンドリングテスト完了");
            println!("  実際のBBCフィード設定でのエラーハンドリング: 成功");
            println!("  HTTP取得エラー時の継続処理: 確認済み");

            Ok(())
        }

        #[tokio::test]
        async fn test_firecrawl_error() -> Result<(), anyhow::Error> {
            let (_container, pool) = setup_test_db().await;
            // エラーシナリオ: RSS取得成功 + 記事取得エラー
            let success_http_client = MockHttpClient::new_success();
            let error_firecrawl_client = MockFirecrawlClient::new_error("記事取得API障害");

            // 実際のrss/link.ymlからBBCグループのフィード数を取得
            let bbc_query = Some(RssLinkQuery::from_group("bbc"));
            let bbc_feeds = search_rss_links(bbc_query.as_ref())?;
            let expected_bbc_feed_count = bbc_feeds.len();

            let result_firecrawl_error = workflow_news(
                &success_http_client,
                &error_firecrawl_client,
                &pool,
                bbc_query.as_ref(),
            )
            .await;

            // ワークフロー全体は成功する（エラーハンドリングにより継続処理）
            assert!(
                result_firecrawl_error.is_ok(),
                "記事取得エラー時もワークフローは成功するべきです"
            );

            // RSS収集は成功するため、article_linksにデータあり
            let rss_count_after_firecrawl_error =
                sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                    .fetch_one(&pool)
                    .await?;
            let expected_rss_count = expected_bbc_feed_count * 3; // 各フィードから3記事生成
            assert_eq!(
                rss_count_after_firecrawl_error.unwrap_or(0),
                expected_rss_count as i64,
                "RSS収集は成功するべきです"
            );

            // 記事取得でエラーが発生した場合、エラー記事として保存される
            // （fetch_article_content 関数は常にOkを返し、エラー情報をstatus_codeとcontentに含める設計）
            let article_count_after_firecrawl_error =
                sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(
                article_count_after_firecrawl_error.unwrap_or(0),
                expected_rss_count as i64, // エラー記事として保存
                "エラー記事として保存されるべきです"
            );

            // エラー記事のステータスコード確認
            let error_articles =
                sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 500")
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(
                error_articles.unwrap_or(0),
                expected_rss_count as i64,
                "すべての記事がエラーステータス(500)で保存されるべきです"
            );

            // エラー記事の内容確認
            let error_content: Option<String> =
                sqlx::query_scalar!("SELECT content FROM articles LIMIT 1")
                    .fetch_optional(&pool)
                    .await?;
            assert!(
                error_content
                    .as_ref()
                    .is_some_and(|content| content.contains("取得エラー:")
                        || content.contains("記事取得APIエラー:")),
                "エラー記事の内容に取得エラーメッセージが含まれるべきです: {:?}",
                error_content
            );

            println!("✅ workflow_news 記事取得エラーテスト完了");
            println!("  BBCフィード数: {}", expected_bbc_feed_count);
            println!(
                "  RSS収集成功: {}件のリンク",
                rss_count_after_firecrawl_error.unwrap_or(0)
            );
            println!(
                "  エラー記事保存: {}件",
                article_count_after_firecrawl_error.unwrap_or(0)
            );
            println!("  記事取得エラー時の適切な処理: 確認済み");

            Ok(())
        }
    }
}
