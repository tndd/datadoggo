use crate::{
    core::feed::{search_feeds, FeedQuery},
    infra::api::{firecrawl::FirecrawlClient, http::HttpClient},
    task::{process_collect_article_links, process_collect_articles},
};
use anyhow::{Context, Result};
use sqlx::PgPool;

/// RSSワークフローのメイン実行関数（依存性を注入）
///
/// 1. feeds.yamlからフィード設定を読み込み
/// 2. 各RSSフィードからリンクを取得してDBに保存
/// 3. 未処理のリンクから記事内容を取得してDBに保存
pub async fn execute_rss_workflow<H: HttpClient, F: FirecrawlClient>(
    http_client: &H,
    firecrawl_client: &F,
    pool: &PgPool,
    group: Option<&str>,
) -> Result<()> {
    match group {
        Some(group_name) => {
            println!("=== RSSワークフロー開始（グループ: {}）===", group_name);
        }
        None => {
            println!("=== RSSワークフロー開始 ===");
        }
    }

    // feeds.yamlからフィード設定を読み込み
    let query = group.map(FeedQuery::from_group);
    let feeds = search_feeds(query).context("フィード設定の読み込みに失敗")?;

    if let Some(group_name) = group {
        if feeds.is_empty() {
            println!(
                "指定されたグループ '{}' のフィードが見つかりませんでした",
                group_name
            );
            return Ok(());
        }
        println!("対象フィード数: {}件", feeds.len());
    } else {
        println!("フィード設定読み込み完了: {}件", feeds.len());
    }

    // 段階1: RSSフィードからリンクを取得
    process_collect_article_links(http_client, &feeds, pool).await?;
    // 段階2: 未処理のリンクから記事内容を取得
    process_collect_articles(firecrawl_client, pool).await?;

    match group {
        Some(group_name) => {
            println!("=== RSSワークフロー完了（グループ: {}）===", group_name);
        }
        None => {
            println!("=== RSSワークフロー完了 ===");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    // composition区分: 複数ドメインが横断するテスト（execute_rss_workflow統合テスト）
    mod composition {
        use super::*;
        use crate::core::feed::{search_feeds, FeedQuery};
        use crate::infra::api::{firecrawl::MockFirecrawlClient, http::MockHttpClient};

        /// 実際のfeeds.yamlを使用して、execute_rss_workflowが正しく動作することをテスト
        /// feed数制限のため、bbcグループのみに制限してテスト
        #[sqlx::test]
        async fn test_execute_rss_workflow(pool: PgPool) -> Result<(), anyhow::Error> {
            // 実際のfeeds.yamlからBBCグループのフィード数を取得
            let bbc_query = Some(FeedQuery::from_group("bbc"));
            let bbc_feeds = search_feeds(bbc_query)?;
            let expected_bbc_feed_count = bbc_feeds.len();

            assert!(
                expected_bbc_feed_count > 0,
                "BBCグループのフィードが見つかりません。feeds.yamlを確認してください"
            );

            println!("BBCフィード数: {}件", expected_bbc_feed_count);

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

            // execute_rss_workflowを実行（実際のfeeds.yamlを使用してBBCグループを指定）
            let result = execute_rss_workflow(
                &mock_http_client,
                &mock_firecrawl_client,
                &pool,
                Some("bbc"),
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
            let expected_rss_count = expected_bbc_feed_count * 3; // 各フィードから3記事生成
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
                    .unwrap()
                    .contains("BBC統合テスト記事の内容です"),
                "記事内容が期待されるモック内容を含んでいません"
            );

            println!("✅ execute_rss_workflow BBC統合テスト完了");
            println!("  BBCフィード数: {}", expected_bbc_feed_count);
            println!("  保存されたRSSリンク数: {}", final_rss_count.unwrap_or(0));
            println!("  保存された記事数: {}", final_article_count.unwrap_or(0));
            println!("  実際のfeeds.yamlからの読み込み: 成功");

            Ok(())
        }

        #[sqlx::test]
        async fn test_execute_rss_workflow_http_error(pool: PgPool) -> Result<(), anyhow::Error> {
            // エラーシナリオ: HTTP取得エラー（実際のfeeds.yaml使用）
            let error_http_client = MockHttpClient::new_error("RSS取得接続エラー");
            let success_firecrawl_client = MockFirecrawlClient::new_success("記事内容");

            let result_http_error = execute_rss_workflow(
                &error_http_client,
                &success_firecrawl_client,
                &pool,
                Some("bbc"),
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

            println!("✅ execute_rss_workflow エラーハンドリングテスト完了");
            println!("  実際のBBCフィード設定でのエラーハンドリング: 成功");
            println!("  HTTP取得エラー時の継続処理: 確認済み");

            Ok(())
        }

        #[sqlx::test]
        async fn test_execute_rss_workflow_firecrawl_error(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // エラーシナリオ: RSS取得成功 + Firecrawl取得エラー
            let success_http_client = MockHttpClient::new_success();
            let error_firecrawl_client = MockFirecrawlClient::new_error("記事取得API障害");

            // 実際のfeeds.yamlからBBCグループのフィード数を取得
            let bbc_query = Some(FeedQuery::from_group("bbc"));
            let bbc_feeds = search_feeds(bbc_query)?;
            let expected_bbc_feed_count = bbc_feeds.len();

            let result_firecrawl_error = execute_rss_workflow(
                &success_http_client,
                &error_firecrawl_client,
                &pool,
                Some("bbc"),
            )
            .await;

            // ワークフロー全体は成功する（エラーハンドリングにより継続処理）
            assert!(
                result_firecrawl_error.is_ok(),
                "Firecrawl取得エラー時もワークフローは成功するべきです"
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
            // （get_article_content_with_client関数は常にOkを返し、エラー情報をstatus_codeとcontentに含める設計）
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
                error_content.is_some() && error_content.unwrap().contains("Firecrawl API エラー:"),
                "エラー記事の内容にFirecrawl API エラーメッセージが含まれるべきです"
            );

            println!("✅ execute_rss_workflow Firecrawlエラーテスト完了");
            println!("  BBCフィード数: {}", expected_bbc_feed_count);
            println!(
                "  RSS収集成功: {}件のリンク",
                rss_count_after_firecrawl_error.unwrap_or(0)
            );
            println!(
                "  エラー記事保存: {}件",
                article_count_after_firecrawl_error.unwrap_or(0)
            );
            println!("  Firecrawlエラー時の適切な処理: 確認済み");

            Ok(())
        }
    }
}
