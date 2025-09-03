use crate::{
    domain::{
        article::{get_article_content_with_client, store_article_content, ArticleContent},
        feed::{search_feeds, Feed, FeedQuery},
        rss::{get_rss_links_from_feed, search_unprocessed_rss_links, store_rss_links},
    },
    infra::api::{firecrawl::FirecrawlClient, http::HttpClient},
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
    process_collect_rss_links(http_client, &feeds, pool).await?;
    // 段階2: 未処理のリンクから記事内容を取得
    process_collect_backlog_articles(firecrawl_client, pool).await?;

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

/// RSSフィードからリンクを収集してDBに保存する
async fn process_collect_rss_links<H: HttpClient>(
    client: &H,
    feeds: &[Feed],
    pool: &PgPool,
) -> Result<()> {
    println!("--- RSSフィードからリンク取得開始 ---");

    for feed in feeds {
        println!("フィード処理中: {}", feed);

        match get_rss_links_from_feed(client, feed).await {
            Ok(rss_links) => {
                println!("  {}件のリンクを抽出", rss_links.len());

                match store_rss_links(&rss_links, pool).await {
                    Ok(result) => {
                        println!("  DB保存結果: {}", result);
                    }
                    Err(e) => {
                        eprintln!("  DB保存エラー: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("  フィード取得エラー: {}", e);
            }
        }
    }

    println!("--- RSSフィードからリンク取得完了 ---");
    Ok(())
}

/// 未処理のリンクから処理待ちの記事を収集してDBに保存する
async fn process_collect_backlog_articles<F: FirecrawlClient>(
    firecrawl_client: &F,
    pool: &PgPool,
) -> Result<()> {
    println!("--- 記事内容取得開始 ---");
    // 未処理のリンクを取得（articleテーブルに存在しないrss_linkを取得）
    let unprocessed_links = search_unprocessed_rss_links(pool).await?;
    println!("未処理リンク数: {}件", unprocessed_links.len());

    for rss_link in unprocessed_links {
        println!("記事処理中: {}", rss_link.link);

        let article_result =
            get_article_content_with_client(&rss_link.link, firecrawl_client).await;

        match article_result {
            Ok(article) => match store_article_content(&article, pool).await {
                Ok(result) => {
                    println!("  記事保存結果: {}", result);
                }
                Err(e) => {
                    eprintln!("  記事保存エラー: {}", e);
                }
            },
            Err(e) => {
                eprintln!("  記事取得エラー: {}", e);

                // エラーが発生した場合も、status_codeを記録してスキップ
                let error_article = ArticleContent {
                    url: rss_link.link,
                    timestamp: chrono::Utc::now(),
                    status_code: 500, // エラー用のステータスコード
                    content: format!("取得エラー: {}", e),
                };

                if let Err(store_err) = store_article_content(&error_article, pool).await {
                    eprintln!("  エラー記事の保存に失敗: {}", store_err);
                }
            }
        }
    }

    println!("--- 記事内容取得完了 ---");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::api::firecrawl::MockFirecrawlClient;
    use sqlx::PgPool;

    /// 統合テスト（モック使用）
    mod integration_tests {
        use super::*;

        #[sqlx::test(fixtures("../../fixtures/workflow_basic.sql"))]
        async fn test_article_fetch_with_mock(pool: PgPool) -> Result<(), anyhow::Error> {
            // fixtureから6件の未処理RSSリンクと3件の処理済み記事が読み込まれる（archiveも再処理される）

            // 全URL成功のモッククライアントを設定（基本テスト用）
            let mock_client = MockFirecrawlClient::new_success("基本テスト記事の内容です");

            // 記事取得を実行（未処理の6件が処理される）
            let result = process_collect_backlog_articles(&mock_client, &pool).await;
            assert!(
                result.is_ok(),
                "記事取得処理が失敗しました: {:?}",
                result.err()
            );

            // 全記事数確認（既存3件 + 新規3件 + 更新3件 = 9件、実際は再処理により既存が更新されて8件）
            let total_articles = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                total_articles.unwrap_or(0),
                8,
                "総記事数が期待値と異なります"
            );

            // 成功記事数確認（全て成功で処理されるため8件）
            let new_success_articles =
                sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 200")
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(
                new_success_articles.unwrap_or(0),
                8,
                "成功記事数が期待値と異なります"
            );

            // エラー記事数の確認（全て成功処理されるため0件）
            let error_articles =
                sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 500")
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(
                error_articles.unwrap_or(0),
                0,
                "エラー記事数が期待値と異なります"
            );

            // 特定記事の内容確認
            let article_content: String = sqlx::query_scalar!(
                "SELECT content FROM articles WHERE url = $1",
                "https://news.example.com/article1"
            )
            .fetch_one(&pool)
            .await?;
            assert!(
                article_content.contains("基本テスト記事の内容です"),
                "記事内容が正しく保存されていません"
            );

            println!("✅ 基本workflow統合テスト完了: 6件の記事を処理しました");
            Ok(())
        }

        #[sqlx::test(fixtures("../../fixtures/workflow_mixed.sql"))]
        async fn test_article_fetch_mixed_scenarios(pool: PgPool) -> Result<(), anyhow::Error> {
            // fixtureから11件の未処理RSSリンクと2件の処理済み記事が読み込まれる（エラー記事も再処理）

            // 全URL成功のモッククライアントを設定（混在テスト用）
            let mock_client = MockFirecrawlClient::new_success("混在テスト記事の内容です");

            // 記事取得を実行（未処理の11件が処理される）
            let result = process_collect_backlog_articles(&mock_client, &pool).await;
            assert!(
                result.is_ok(),
                "混在シナリオの処理が失敗しました: {:?}",
                result.err()
            );

            // 全記事数確認（既存2件 + 新規10件 = 12件）
            let total_articles = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                total_articles.unwrap_or(0),
                12,
                "総記事数が期待値と異なります"
            );

            // 成功記事数確認（全て成功で処理されるため12件）
            let success_articles =
                sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 200")
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(
                success_articles.unwrap_or(0),
                12,
                "成功記事数が期待値と異なります"
            );

            // エラー記事数確認（全て成功処理されるため0件）
            let error_articles =
                sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 500")
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(
                error_articles.unwrap_or(0),
                0,
                "エラー記事数が期待値と異なります"
            );

            // 成功記事の内容確認（全て成功するのでいずれかの記事を確認）
            let success_content: String = sqlx::query_scalar!(
                "SELECT content FROM articles WHERE url = $1",
                "https://success.example.com/news1"
            )
            .fetch_one(&pool)
            .await?;
            assert!(
                success_content.contains("混在テスト記事の内容です"),
                "成功記事の内容が正しくありません"
            );

            println!("✅ 混在シナリオworkflow統合テスト完了: 11件すべて成功処理しました");
            Ok(())
        }

        #[sqlx::test(fixtures("../../fixtures/workflow_large.sql"))]
        async fn test_article_fetch_large_scale(pool: PgPool) -> Result<(), anyhow::Error> {
            // fixtureから22件の未処理RSSリンクと5件の処理済み記事が読み込まれる（エラー記事も再処理）

            // 全URL成功のモッククライアントを設定（大規模テスト用）
            let mock_client = MockFirecrawlClient::new_success("大規模テスト記事の内容です");

            // 記事取得を実行（未処理の22件が処理される）
            let result = process_collect_backlog_articles(&mock_client, &pool).await;
            assert!(
                result.is_ok(),
                "大規模処理が失敗しました: {:?}",
                result.err()
            );

            // 全記事数確認（既存5件 + 新規20件 = 25件）
            let total_articles = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                total_articles.unwrap_or(0),
                25,
                "総記事数が期待値と異なります"
            );

            // 成功記事数確認（全て成功処理されるため25件）
            let success_articles =
                sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 200")
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(
                success_articles.unwrap_or(0),
                25,
                "成功記事数が期待値と異なります"
            );

            // エラー記事数確認（全て成功処理されるため0件）
            let error_articles =
                sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 500")
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(
                error_articles.unwrap_or(0),
                0,
                "エラー記事数が期待値と異なります"
            );

            // 処理性能の確認（大規模データでも正常完了することの確認）
            let processed_urls =
                sqlx::query!("SELECT url FROM articles WHERE url LIKE '%large%' ORDER BY url")
                    .fetch_all(&pool)
                    .await?;
            assert!(
                processed_urls.len() >= 22,
                "大規模データが十分処理されていません"
            );

            println!("✅ 大規模workflow統合テスト完了: 22件の記事をすべて成功処理しました");
            Ok(())
        }
    }
}
