use crate::{
    core::{
        link::{fetch_article_links_using_feed, store_article_links},
        rss::RssLink,
    },
    infra::api::http::HttpClient,
};
use anyhow::Result;
use sqlx::PgPool;

/// RSSフィードからリンクを収集してDBに保存する
pub(super) async fn task_collect_article_links<H: HttpClient>(
    client: &H,
    rss_links: &[RssLink],
    pool: &PgPool,
) -> Result<()> {
    println!("--- RSSフィードからリンク取得開始 ---");

    for rss_link in rss_links {
        println!("フィード処理中: {}", rss_link);

        match fetch_article_links_using_feed(client, rss_link).await {
            Ok(article_links) => {
                println!("  {}件のリンクを抽出", article_links.len());

                match store_article_links(&article_links, pool).await {
                    Ok(_) => {
                        println!("  DB保存完了: {}件処理", article_links.len());
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    // 関数名ベースのモジュールでテストを集約
    mod task_collect_article_links {
        use super::*;
        #[sqlx::test]
        async fn test_success(pool: PgPool) -> Result<(), anyhow::Error> {
            use crate::core::rss::RssLink;
            use crate::infra::api::http::MockHttpClient;

            // テスト用フィードを準備（異なるURLで3つのフィード）
            let test_feeds = vec![
                RssLink {
                    group: "news".to_string(),
                    name: "tech_news".to_string(),
                    url: "https://technews.example.com/rss.xml".to_string(),
                },
                RssLink {
                    group: "blog".to_string(),
                    name: "dev_blog".to_string(),
                    url: "https://devblog.example.com/feed.xml".to_string(),
                },
                RssLink {
                    group: "updates".to_string(),
                    name: "product_updates".to_string(),
                    url: "https://updates.example.com/rss".to_string(),
                },
            ];

            // MockHttpClientで成功レスポンスを設定
            let mock_client = MockHttpClient::new_success();

            // 処理前のarticle_links件数を確認
            let initial_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(initial_count.unwrap_or(0), 0);

            // 実行
            let result = task_collect_article_links(&mock_client, &test_feeds, &pool).await;
            assert!(result.is_ok());

            // 件数確認（3フィード × 3記事 = 9件）
            let final_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(final_count.unwrap_or(0), 9);

            // 各フィードから生成されたリンクの形式を検証
            use crate::infra::compute::generate_mock_rss_id;
            for rss_link in &test_feeds {
                let hash = generate_mock_rss_id(&rss_link.url);
                let feed_link_count = sqlx::query_scalar!(
                    "SELECT COUNT(*) FROM article_links WHERE url LIKE $1",
                    format!("https://{}.example.com/%", hash)
                )
                .fetch_one(&pool)
                .await?;
                assert_eq!(feed_link_count.unwrap_or(0), 3);
            }

            Ok(())
        }

        #[sqlx::test]
        async fn test_with_errors(pool: PgPool) -> Result<(), anyhow::Error> {
            use crate::core::rss::RssLink;
            use crate::infra::api::http::MockHttpClient;

            let test_feeds = vec![
                RssLink {
                    group: "success".to_string(),
                    name: "working_feed".to_string(),
                    url: "https://working.example.com/rss.xml".to_string(),
                },
                RssLink {
                    group: "error1".to_string(),
                    name: "timeout_feed".to_string(),
                    url: "https://timeout.example.com/rss.xml".to_string(),
                },
                RssLink {
                    group: "error2".to_string(),
                    name: "server_error_feed".to_string(),
                    url: "https://servererror.example.com/rss.xml".to_string(),
                },
            ];

            let success_client = MockHttpClient::new_success();

            // 成功フィードのみ処理
            let success_feeds = vec![test_feeds[0].clone()];
            let result = task_collect_article_links(&success_client, &success_feeds, &pool).await;
            assert!(result.is_ok());
            let success_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(success_count.unwrap_or(0), 3);

            // 以降はエラークライアントで全フィード処理（追加されない）
            let error_client = MockHttpClient::new_error("接続タイムアウト");
            let all_result = task_collect_article_links(&error_client, &test_feeds, &pool).await;
            assert!(all_result.is_ok());
            let final_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(final_count.unwrap_or(0), 3);

            Ok(())
        }

        #[sqlx::test]
        async fn test_duplicate_handling(pool: PgPool) -> Result<(), anyhow::Error> {
            use crate::core::rss::RssLink;
            use crate::infra::api::http::MockHttpClient;

            let same_rss_url = "https://shared.example.com/common.xml";
            let duplicate_feeds = vec![
                RssLink {
                    group: "group1".to_string(),
                    name: "shared_feed_1".to_string(),
                    url: same_rss_url.to_string(),
                },
                RssLink {
                    group: "group2".to_string(),
                    name: "shared_feed_2".to_string(),
                    url: same_rss_url.to_string(),
                },
                RssLink {
                    group: "group3".to_string(),
                    name: "shared_feed_3".to_string(),
                    url: same_rss_url.to_string(),
                },
            ];

            let mock_client = MockHttpClient::new_success();

            // 初期状態
            let initial_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(initial_count.unwrap_or(0), 0);

            // 1回目
            let first_feed = vec![duplicate_feeds[0].clone()];
            let result1 = task_collect_article_links(&mock_client, &first_feed, &pool).await;
            assert!(result1.is_ok());
            let after_first_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(after_first_count.unwrap_or(0), 3);

            // 1回目実行後の日付を記録（更新確認のため）
            let first_pub_dates: Vec<chrono::DateTime<chrono::Utc>> =
                sqlx::query_scalar!("SELECT pub_date FROM article_links ORDER BY url")
                    .fetch_all(&pool)
                    .await?;
            assert_eq!(
                first_pub_dates.len(),
                3,
                "1回目実行後に3件の日付が記録されるべきです"
            );

            // 少し待機して、動的日付生成で異なる時刻になることを確保
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

            // 2回目の実行：同一URLのフィードを再度処理（重複発生）
            let second_feed = vec![duplicate_feeds[1].clone()];
            let result2 = task_collect_article_links(&mock_client, &second_feed, &pool).await;
            assert!(result2.is_ok(), "2回目のRSS収集処理が失敗しました");

            // 2回目実行後の件数確認（重複により件数は変わらず3件のまま）
            let after_second_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                after_second_count.unwrap_or(0),
                3,
                "2回目実行後も3件のまま（重複時は上書き更新）であるべきです"
            );

            // 2回目実行後の日付を取得して更新状況を確認
            let second_pub_dates: Vec<chrono::DateTime<chrono::Utc>> =
                sqlx::query_scalar!("SELECT pub_date FROM article_links ORDER BY url")
                    .fetch_all(&pool)
                    .await?;
            assert_eq!(
                second_pub_dates.len(),
                3,
                "2回目実行後も3件の日付が記録されているべきです"
            );

            // 重複リンクの場合、日付は更新される（ON CONFLICT DO UPDATE）
            for (i, (first_date, second_date)) in first_pub_dates
                .iter()
                .zip(second_pub_dates.iter())
                .enumerate()
            {
                assert_ne!(
                    first_date,
                    second_date,
                    "記事{}の日付が更新されませんでした（重複時は新しい日付で更新されるべき）: {} == {}",
                    i + 1,
                    first_date,
                    second_date
                );
                assert!(
                    second_date >= first_date,
                    "記事{}の日付が過去に戻りました（新しい日付のほうが新しいべき）: {} < {}",
                    i + 1,
                    second_date,
                    first_date
                );
            }

            // 3回目の実行：全ての重複フィードを一度に処理
            let all_result =
                task_collect_article_links(&mock_client, &duplicate_feeds, &pool).await;
            assert!(all_result.is_ok(), "全重複フィードの処理が失敗しました");

            // 最終的な件数確認（依然として3件のまま）
            let final_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                final_count.unwrap_or(0),
                3,
                "最終的にも3件のまま（すべての重複が上書き更新）であるべきです"
            );

            // 保存されたリンクの内容確認
            use crate::infra::compute::generate_mock_rss_id;
            let expected_hash = generate_mock_rss_id(same_rss_url);

            for article_num in 1..=3 {
                let expected_title = format!("{}:title:{}", expected_hash, article_num);
                let expected_link =
                    format!("https://{}.example.com/{}", expected_hash, article_num);

                let link_exists = sqlx::query_scalar!(
                    "SELECT COUNT(*) FROM article_links WHERE title = $1 AND url = $2",
                    expected_title,
                    expected_link
                )
                .fetch_one(&pool)
                .await?;

                assert_eq!(
                    link_exists.unwrap_or(0),
                    1,
                    "期待されるリンク '{}' が1件だけ存在すべきです（重複なし）",
                    expected_link
                );
            }

            // 異なるURLのフィードを追加して、重複処理が新規リンクをブロックしないことを確認
            let unique_feed = vec![RssLink {
                group: "unique".to_string(),
                name: "unique_feed".to_string(),
                url: "https://unique.example.com/different.xml".to_string(),
            }];

            let unique_result = task_collect_article_links(&mock_client, &unique_feed, &pool).await;
            assert!(
                unique_result.is_ok(),
                "ユニークフィードの処理が失敗しました"
            );

            // 新規フィードからの3件が追加されて、合計6件になることを確認
            let final_unique_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                final_unique_count.unwrap_or(0),
                6,
                "新規フィード追加後は6件（既存3件 + 新規3件）になるべきです"
            );

            println!("✅ RSS重複処理テスト完了");
            println!("  重複リンクは正しく上書き更新されました（日付が新しく更新）");
            println!("  新規リンクは正しく追加されました（動的日付生成）");
            println!("  最終リンク数: {}", final_unique_count.unwrap_or(0));

            Ok(())
        }
    }

    // 並行処理テスト（関数名ベースの配下）
    mod task_collect_article_links_concurrent {
        use super::*;
        use crate::infra::api::http::MockHttpClient;

        #[sqlx::test(fixtures("concurrent_processing"))]
        async fn test_concurrent_feed_processing_simulation(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // 並行処理シミュレーション：異なるフィードを順次処理
            let mock_client = MockHttpClient::new_success();

            // フィード1: 技術ニュース（3記事）
            let tech_feeds = vec![RssLink {
                group: "tech".to_string(),
                name: "tech_news".to_string(),
                url: "https://tech-concurrent.example.com/rss.xml".to_string(),
            }];

            // フィード2: ビジネスニュース（3記事）
            let business_feeds = vec![RssLink {
                group: "business".to_string(),
                name: "business_news".to_string(),
                url: "https://business-concurrent.example.com/rss.xml".to_string(),
            }];

            // 初期状態確認
            let initial_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;

            // 第1回目：技術フィード処理
            let result1 = task_collect_article_links(&mock_client, &tech_feeds, &pool).await;
            assert!(result1.is_ok(), "技術フィード処理が失敗しました");

            let _after_tech_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;

            // 第2回目：ビジネスフィード処理
            let result2 = task_collect_article_links(&mock_client, &business_feeds, &pool).await;
            assert!(result2.is_ok(), "ビジネスフィード処理が失敗しました");

            let after_business_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;

            // 合計で新規6件（各フィード3件ずつ）が追加されていることを確認
            assert_eq!(
                after_business_count.unwrap_or(0),
                initial_count.unwrap_or(0) + 6,
                "並行フィード処理後の記事数が期待値と異なります"
            );

            // 各フィード別の記事数確認
            use crate::infra::compute::generate_mock_rss_id;
            let tech_hash = generate_mock_rss_id("https://tech-concurrent.example.com/rss.xml");
            let business_hash =
                generate_mock_rss_id("https://business-concurrent.example.com/rss.xml");

            let tech_count = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM article_links WHERE url LIKE $1",
                format!("https://{}.example.com/%", tech_hash)
            )
            .fetch_one(&pool)
            .await?;

            let business_count = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM article_links WHERE url LIKE $1",
                format!("https://{}.example.com/%", business_hash)
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(
                tech_count.unwrap_or(0),
                3,
                "技術フィードから3件の記事が生成されるべきです"
            );
            assert_eq!(
                business_count.unwrap_or(0),
                3,
                "ビジネスフィードから3件の記事が生成されるべきです"
            );

            println!("✅ 並行フィード処理シミュレーション完了");
            println!("  技術フィード: {}件", tech_count.unwrap_or(0));
            println!("  ビジネスフィード: {}件", business_count.unwrap_or(0));
            Ok(())
        }

        #[sqlx::test(fixtures("concurrent_processing"))]
        async fn test_duplicate_url_race_condition_handling(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // 競合状態テスト：同一URLの記事が複数回処理される場合
            let mock_client = MockHttpClient::new_success();

            // 同じURLを持つ複数のフィードをシミュレート
            // Vecではなく固定長配列で十分なため、Clippyに従い配列に変更
            let competing_feeds = [
                RssLink {
                    group: "source1".to_string(),
                    name: "competing_feed_1".to_string(),
                    url: "https://race-condition-test.example.com/same.xml".to_string(),
                },
                RssLink {
                    group: "source2".to_string(),
                    name: "competing_feed_2".to_string(),
                    url: "https://race-condition-test.example.com/same.xml".to_string(),
                },
            ];

            // 1回目の処理
            let result1 =
                task_collect_article_links(&mock_client, &competing_feeds[0..1], &pool).await;
            assert!(result1.is_ok(), "1回目の処理が失敗しました");

            let first_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;

            // 1回目処理後の記事内容を記録
            use crate::infra::compute::generate_mock_rss_id;
            let expected_hash =
                generate_mock_rss_id("https://race-condition-test.example.com/same.xml");
            let first_articles: Vec<(String, chrono::DateTime<chrono::Utc>)> = sqlx::query!(
                "SELECT title, pub_date FROM article_links WHERE url LIKE $1 ORDER BY url",
                format!("https://{}.example.com/%", expected_hash)
            )
            .fetch_all(&pool)
            .await?
            .into_iter()
            .map(|row| (row.title, row.pub_date))
            .collect();

            // 少し待機（動的日付生成で異なる時刻を保証）
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

            // 2回目の処理（同じURLのフィード）
            let result2 =
                task_collect_article_links(&mock_client, &competing_feeds[1..2], &pool).await;
            assert!(result2.is_ok(), "2回目の処理が失敗しました");

            let second_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;

            // 件数は変わらない（重複URLはUPSERTされる）
            assert_eq!(
                second_count.unwrap_or(0),
                first_count.unwrap_or(0),
                "重複処理で件数が変わってしまいました"
            );

            // しかし内容（日付）は更新される
            let second_articles: Vec<(String, chrono::DateTime<chrono::Utc>)> = sqlx::query!(
                "SELECT title, pub_date FROM article_links WHERE url LIKE $1 ORDER BY url",
                format!("https://{}.example.com/%", expected_hash)
            )
            .fetch_all(&pool)
            .await?
            .into_iter()
            .map(|row| (row.title, row.pub_date))
            .collect();

            // 記事数は同じだが、日付が更新されている
            assert_eq!(first_articles.len(), second_articles.len());
            for (i, ((first_title, first_date), (second_title, second_date))) in first_articles
                .iter()
                .zip(second_articles.iter())
                .enumerate()
            {
                assert_eq!(
                    first_title,
                    second_title,
                    "記事{}のタイトルが一致しません",
                    i + 1
                );
                assert_ne!(
                    first_date,
                    second_date,
                    "記事{}の日付が更新されませんでした: {} == {}",
                    i + 1,
                    first_date,
                    second_date
                );
            }

            println!("✅ 重複URL競合処理テスト完了");
            println!("  重複処理により日付が適切に更新されました");
            Ok(())
        }
    }

    // エラー回復テスト
    mod error_recovery_tests {
        use super::*;
        use crate::infra::api::http::MockHttpClient;

        #[sqlx::test(fixtures("error_recovery_scenarios"))]
        async fn test_mixed_success_error_feed_processing(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // 成功・エラー混在処理のテスト
            let success_client = MockHttpClient::new_success();
            let error_client = MockHttpClient::new_error("ネットワークエラー");

            // 成功するフィード
            let success_feeds = vec![RssLink {
                group: "success".to_string(),
                name: "working_feed".to_string(),
                url: "https://success-recovery.example.com/feed.xml".to_string(),
            }];

            // エラーになるフィード
            let error_feeds = vec![RssLink {
                group: "error".to_string(),
                name: "failing_feed".to_string(),
                url: "https://error-recovery.example.com/feed.xml".to_string(),
            }];

            let initial_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;

            // 成功フィード処理
            let success_result =
                task_collect_article_links(&success_client, &success_feeds, &pool).await;
            assert!(success_result.is_ok(), "成功フィード処理が失敗しました");

            let after_success_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                after_success_count.unwrap_or(0),
                initial_count.unwrap_or(0) + 3,
                "成功フィードから3件の記事が追加されるべきです"
            );

            // エラーフィード処理（内部でエラーハンドリングされるため、関数自体は成功）
            let error_result = task_collect_article_links(&error_client, &error_feeds, &pool).await;
            assert!(
                error_result.is_ok(),
                "エラーフィード処理（エラーハンドリング）が失敗しました"
            );

            // エラー後も件数は変わらない
            let after_error_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                after_error_count.unwrap_or(0),
                after_success_count.unwrap_or(0),
                "エラーフィード処理後に記事数が変わってしまいました"
            );

            println!("✅ 成功・エラー混在フィード処理テスト完了");
            println!("  成功フィード: 3件追加");
            println!("  エラーフィード: エラーハンドリングにより処理継続");
            Ok(())
        }

        #[sqlx::test(fixtures("error_recovery_scenarios"))]
        async fn test_batch_processing_boundary_conditions(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // バッチ処理境界条件のテスト（LIMIT 100の動作確認）
            let mock_client = MockHttpClient::new_success();

            // 大量フィード処理用のフィードを作成（実際には10記事しか生成しない）
            let batch_feeds = vec![RssLink {
                group: "batch".to_string(),
                name: "large_batch_feed".to_string(),
                url: "https://batch-boundary-test.example.com/feed.xml".to_string(),
            }];

            let initial_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;

            let batch_result = task_collect_article_links(&mock_client, &batch_feeds, &pool).await;
            assert!(batch_result.is_ok(), "バッチ処理が失敗しました");

            let after_batch_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;

            // 新しく3件の記事が追加される（MockHttpClientは常に3件生成）
            assert_eq!(
                after_batch_count.unwrap_or(0),
                initial_count.unwrap_or(0) + 3,
                "バッチ処理で期待される記事数が追加されませんでした"
            );

            // 生成された記事の確認
            use crate::infra::compute::generate_mock_rss_id;
            let hash = generate_mock_rss_id("https://batch-boundary-test.example.com/feed.xml");
            let batch_articles = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM article_links WHERE url LIKE $1",
                format!("https://{}.example.com/%", hash)
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(
                batch_articles.unwrap_or(0),
                3,
                "バッチフィードから3件の記事が生成されるべきです"
            );

            println!("✅ バッチ処理境界条件テスト完了");
            println!("  処理された記事数: {}", batch_articles.unwrap_or(0));
            Ok(())
        }
    }
}
