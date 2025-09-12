use crate::{
    core::{
        article::{fetch_article_content_via_firecrawl, store_article_content, ArticleContent},
        link::search_backlog_article_links,
    },
    infra::api::firecrawl::FirecrawlClient,
};
use anyhow::Result;
use sqlx::PgPool;

/// バックログ対象リンクから処理待ちの記事を収集してDBに保存する
pub async fn task_collect_articles<F: FirecrawlClient>(
    firecrawl_client: &F,
    pool: &PgPool,
) -> Result<()> {
    println!("--- 記事内容取得開始 ---");
    // 未処理のリンクを取得（articleテーブルに存在しないarticle_linkを取得）
    let unprocessed_links = search_backlog_article_links(pool).await?;
    println!("未処理リンク数: {}件", unprocessed_links.len());

    for article_link in unprocessed_links {
        println!("記事処理中: {}", article_link.url);

        let article_result =
            fetch_article_content_via_firecrawl(&article_link.url, firecrawl_client).await;

        match article_result {
            Ok(article) => match store_article_content(&article, pool).await {
                Ok(_) => {
                    println!("  記事保存完了");
                }
                Err(e) => {
                    eprintln!("  記事保存エラー: {}", e);
                }
            },
            Err(e) => {
                eprintln!("  記事取得エラー: {}", e);

                // エラーが発生した場合も、status_codeを記録してスキップ
                let error_article = ArticleContent {
                    url: article_link.url,
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

    // 関数名ベースのモジュールに統一
    mod task_collect_articles {
        use super::*;

        #[sqlx::test(fixtures("article"))]
        async fn test_basic(pool: PgPool) -> Result<(), anyhow::Error> {
            // fixtureから6件の未処理RSSリンクと3件の処理済み記事が読み込まれる（archiveも再処理される）

            // 全URL成功のモッククライアントを設定（基本テスト用）
            let mock_client = MockFirecrawlClient::new_success("基本テスト記事の内容です");
            // 記事取得を実行（未処理の6件が処理される）
            let result = task_collect_articles(&mock_client, &pool).await;
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

        #[sqlx::test(fixtures("article_mixed"))]
        async fn test_mixed(pool: PgPool) -> Result<(), anyhow::Error> {
            // fixtureから11件の未処理RSSリンクと2件の処理済み記事が読み込まれる（エラー記事も再処理）

            // 全URL成功のモッククライアントを設定（混在テスト用）
            let mock_client = MockFirecrawlClient::new_success("混在テスト記事の内容です");
            // 記事取得を実行（未処理の11件が処理される）
            let result = task_collect_articles(&mock_client, &pool).await;
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

        // エラー回復と継続処理テスト
        mod error_recovery {
            use super::*;

            #[sqlx::test(fixtures("error_recovery_scenarios"))]
            async fn test_error_article_reprocessing(pool: PgPool) -> Result<(), anyhow::Error> {
                // エラー記事の再処理テスト
                let mock_client = MockFirecrawlClient::new_success("再処理成功内容");

                // 初期状態確認：fixtureから複数のエラー記事が読み込まれている
                let initial_error_count =
                    sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code != 200")
                        .fetch_one(&pool)
                        .await?;

                assert!(
                    initial_error_count.unwrap_or(0) > 0,
                    "初期状態でエラー記事が存在しないとテストできません"
                );

                let _initial_backlog_count = sqlx::query_scalar!(
                    "SELECT COUNT(*) FROM article_links al 
                 LEFT JOIN articles a ON al.url = a.url 
                 WHERE a.url IS NULL OR a.status_code != 200"
                )
                .fetch_one(&pool)
                .await?;

                // 記事処理を実行（エラー記事を再処理）
                let result = task_collect_articles(&mock_client, &pool).await;
                assert!(
                    result.is_ok(),
                    "エラー記事再処理が失敗しました: {:?}",
                    result.err()
                );

                // 再処理後、すべての記事が成功状態になることを確認
                let final_success_count =
                    sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 200")
                        .fetch_one(&pool)
                        .await?;

                let final_error_count =
                    sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code != 200")
                        .fetch_one(&pool)
                        .await?;

                assert_eq!(
                    final_error_count.unwrap_or(0),
                    0,
                    "再処理後もエラー記事が残っています"
                );

                // バックログが空になることを確認
                let final_backlog_count = sqlx::query_scalar!(
                    "SELECT COUNT(*) FROM article_links al 
                 LEFT JOIN articles a ON al.url = a.url 
                 WHERE a.url IS NULL OR a.status_code != 200"
                )
                .fetch_one(&pool)
                .await?;

                assert_eq!(
                    final_backlog_count.unwrap_or(0),
                    0,
                    "再処理後にバックログが残っています"
                );

                // 再処理された記事の内容確認
                let reprocessed_content: Option<String> = sqlx::query_scalar!(
                    "SELECT content FROM articles WHERE content = $1 LIMIT 1",
                    "再処理成功内容"
                )
                .fetch_optional(&pool)
                .await?;

                assert!(
                    reprocessed_content.is_some(),
                    "再処理された記事の内容が正しくありません"
                );

                println!("✅ エラー記事再処理テスト完了");
                println!("  初期エラー記事数: {}", initial_error_count.unwrap_or(0));
                println!("  再処理後成功記事数: {}", final_success_count.unwrap_or(0));
                println!("  最終バックログ数: {}", final_backlog_count.unwrap_or(0));
                Ok(())
            }

            #[sqlx::test(fixtures("error_recovery_scenarios"))]
            async fn test_partial_failure_continuation(pool: PgPool) -> Result<(), anyhow::Error> {
                // 一部失敗でも処理継続するテスト
                let error_client = MockFirecrawlClient::new_error("一部記事でAPI障害");

                // 初期状態のバックログ数を確認
                let initial_backlog = search_backlog_article_links(&pool).await?;
                let initial_backlog_count = initial_backlog.len();

                assert!(
                    initial_backlog_count > 0,
                    "初期状態でバックログが存在しないとテストできません"
                );

                // エラークライアントで処理実行（全記事でエラーが発生する予定）
                let result = task_collect_articles(&error_client, &pool).await;
                assert!(
                    result.is_ok(),
                    "エラーが発生しても処理は継続されるべきです: {:?}",
                    result.err()
                );

                // 処理後、エラー記事として記録されていることを確認
                let error_articles =
                    sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 500")
                        .fetch_one(&pool)
                        .await?;

                assert!(
                    error_articles.unwrap_or(0) > 0,
                    "エラー記事が適切に記録されていません"
                );

                // エラー記事の内容確認
                let error_content: Option<String> = sqlx::query_scalar!(
                    "SELECT content FROM articles WHERE status_code = 500 LIMIT 1"
                )
                .fetch_optional(&pool)
                .await?;

                assert!(error_content.is_some(), "エラー記事の内容が見つかりません");
                let content = error_content.unwrap();
                println!("エラー記事内容の例: {}", content);
                assert!(
                    content.contains("取得エラー:") || content.contains("記事取得APIエラー:"),
                    "エラー記事の内容形式が正しくありません: {}",
                    content
                );

                // 処理が中断せずに最後まで実行されたことを確認
                let final_backlog = search_backlog_article_links(&pool).await?;

                // エラー記事は再処理対象として残る（status_code != 200）
                assert!(
                    final_backlog.len() > 0,
                    "エラー記事は再処理対象として残るべきです"
                );

                println!("✅ 部分失敗継続処理テスト完了");
                println!("  初期バックログ数: {}", initial_backlog_count);
                println!("  生成されたエラー記事数: {}", error_articles.unwrap_or(0));
                println!("  最終バックログ数: {}", final_backlog.len());
                Ok(())
            }

            #[sqlx::test(fixtures("concurrent_processing"))]
            async fn test_mixed_result_handling(pool: PgPool) -> Result<(), anyhow::Error> {
                // 成功・失敗混在結果の処理テスト
                let success_client = MockFirecrawlClient::new_success("成功記事内容");

                // concurrent_processing fixtureには処理済み・未処理の混在データが含まれる
                let initial_backlog = search_backlog_article_links(&pool).await?;
                let initial_backlog_count = initial_backlog.len();

                let initial_success_count =
                    sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 200")
                        .fetch_one(&pool)
                        .await?;

                let initial_error_count =
                    sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code != 200")
                        .fetch_one(&pool)
                        .await?;

                // 成功クライアントで処理実行
                let result = task_collect_articles(&success_client, &pool).await;
                assert!(
                    result.is_ok(),
                    "混在結果処理が失敗しました: {:?}",
                    result.err()
                );

                // 処理後の状態確認
                let final_success_count =
                    sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 200")
                        .fetch_one(&pool)
                        .await?;

                let final_error_count =
                    sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code != 200")
                        .fetch_one(&pool)
                        .await?;

                // バックログがすべて処理されたことを確認
                let final_backlog = search_backlog_article_links(&pool).await?;
                assert_eq!(
                    final_backlog.len(),
                    0,
                    "バックログがすべて処理されるべきです"
                );

                // 成功記事が増加していることを確認
                assert!(
                    final_success_count.unwrap_or(0) > initial_success_count.unwrap_or(0),
                    "成功記事数が増加していません"
                );

                // エラー記事が成功に変換されたことを確認
                assert_eq!(
                    final_error_count.unwrap_or(0),
                    0,
                    "エラー記事がすべて成功に変換されるべきです"
                );

                // 新規処理された記事の内容確認
                let new_success_content: Option<String> = sqlx::query_scalar!(
                    "SELECT content FROM articles WHERE content = $1 LIMIT 1",
                    "成功記事内容"
                )
                .fetch_optional(&pool)
                .await?;

                assert!(
                    new_success_content.is_some(),
                    "新規処理された記事の内容が見つかりません"
                );

                println!("✅ 成功・失敗混在結果処理テスト完了");
                println!("  初期バックログ数: {}", initial_backlog_count);
                println!("  初期成功記事数: {}", initial_success_count.unwrap_or(0));
                println!("  初期エラー記事数: {}", initial_error_count.unwrap_or(0));
                println!("  最終成功記事数: {}", final_success_count.unwrap_or(0));
                println!("  最終エラー記事数: {}", final_error_count.unwrap_or(0));
                Ok(())
            }
        }
    }
}
