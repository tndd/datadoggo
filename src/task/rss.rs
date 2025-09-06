use crate::{
    core::{
        feed::Feed,
        rss::{get_article_links_from_feed, store_article_links},
    },
    infra::api::http::HttpClient,
};
use anyhow::Result;
use sqlx::PgPool;

/// RSSフィードからリンクを収集してDBに保存する
pub async fn task_collect_article_links<H: HttpClient>(
    client: &H,
    feeds: &[Feed],
    pool: &PgPool,
) -> Result<()> {
    println!("--- RSSフィードからリンク取得開始 ---");

    for feed in feeds {
        println!("フィード処理中: {}", feed);

        match get_article_links_from_feed(client, feed).await {
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

    #[sqlx::test]
    async fn test_task_collect_article_links_success(pool: PgPool) -> Result<(), anyhow::Error> {
        use crate::core::feed::Feed;
        use crate::infra::api::http::MockHttpClient;

        // テスト用フィードを準備（異なるURLで3つのフィード）
        let test_feeds = vec![
            Feed {
                group: "news".to_string(),
                name: "tech_news".to_string(),
                rss_link: "https://technews.example.com/rss.xml".to_string(),
            },
            Feed {
                group: "blog".to_string(),
                name: "dev_blog".to_string(),
                rss_link: "https://devblog.example.com/feed.xml".to_string(),
            },
            Feed {
                group: "updates".to_string(),
                name: "product_updates".to_string(),
                rss_link: "https://updates.example.com/rss".to_string(),
            },
        ];

        // MockHttpClientで成功レスポンスを設定
        let mock_client = MockHttpClient::new_success();

        // 処理前のarticle_links件数を確認
        let initial_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            initial_count.unwrap_or(0),
            0,
            "初期状態でarticle_linksが空ではありません"
        );

        // task_collect_article_linksを実行
        let result = task_collect_article_links(&mock_client, &test_feeds, &pool).await;
        assert!(
            result.is_ok(),
            "RSS収集処理が失敗しました: {:?}",
            result.err()
        );

        // 処理後のarticle_links件数を確認（3フィード × 3記事 = 9件）
        let final_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            final_count.unwrap_or(0),
            9,
            "期待されるarticle_links件数と異なります"
        );

        // 各フィードから生成されたリンクの形式を検証
        use crate::infra::compute::generate_mock_rss_id;

        for feed in &test_feeds {
            let hash = generate_mock_rss_id(&feed.rss_link);

            // 各フィードから3件のリンクが生成されていることを確認
            let feed_link_count = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM article_links WHERE url LIKE $1",
                format!("https://{}.example.com/%", hash)
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(
                feed_link_count.unwrap_or(0),
                3,
                "フィード {} から3件のリンクが生成されるべきです",
                feed
            );

            // タイトルの形式検証（{hash}:title:X の形式）
            for article_num in 1..=3 {
                let expected_title = format!("{}:title:{}", hash, article_num);
                let expected_link = format!("https://{}.example.com/{}", hash, article_num);

                let title_exists = sqlx::query_scalar!(
                    "SELECT COUNT(*) FROM article_links WHERE title = $1 AND url = $2",
                    expected_title,
                    expected_link
                )
                .fetch_one(&pool)
                .await?;

                assert_eq!(
                    title_exists.unwrap_or(0),
                    1,
                    "期待されるタイトル '{}' とリンク '{}' の組み合わせが見つかりません",
                    expected_title,
                    expected_link
                );
            }
        }

        // 動的生成された日付が適切な範囲に設定されていることを確認（3日前～今日）
        let date_count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM article_links WHERE pub_date BETWEEN $1 AND $2",
            chrono::Utc::now() - chrono::Duration::days(3),
            chrono::Utc::now() + chrono::Duration::hours(1)
        )
        .fetch_one(&pool)
        .await?;

        assert_eq!(
            date_count.unwrap_or(0),
            9,
            "すべてのリンクの日付が動的生成範囲（3日以内）にありません"
        );

        println!("✅ RSS収集基本テスト完了");
        println!("  処理されたフィード数: {}", test_feeds.len());
        println!("  保存されたリンク数: {}", final_count.unwrap_or(0));

        Ok(())
    }

    #[sqlx::test]
    async fn test_task_collect_article_links_with_errors(
        pool: PgPool,
    ) -> Result<(), anyhow::Error> {
        use crate::core::feed::Feed;
        use crate::infra::api::http::MockHttpClient;

        // 成功フィード1つ + エラーフィード2つを準備
        let test_feeds = vec![
            Feed {
                group: "success".to_string(),
                name: "working_feed".to_string(),
                rss_link: "https://working.example.com/rss.xml".to_string(),
            },
            Feed {
                group: "error1".to_string(),
                name: "timeout_feed".to_string(),
                rss_link: "https://timeout.example.com/rss.xml".to_string(),
            },
            Feed {
                group: "error2".to_string(),
                name: "server_error_feed".to_string(),
                rss_link: "https://servererror.example.com/rss.xml".to_string(),
            },
        ];

        // 成功クライアントで正常フィードを処理
        let success_client = MockHttpClient::new_success();

        // task_collect_article_linksは内部的にはエラーを握り潰して継続処理するため、
        // 個別にテストする必要がある

        // 1. 成功フィードのテスト
        let success_feeds = vec![test_feeds[0].clone()];
        let result = task_collect_article_links(&success_client, &success_feeds, &pool).await;
        assert!(result.is_ok(), "成功フィードの処理が失敗しました");

        // 成功フィードからの3件のリンクが保存されることを確認
        let success_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            success_count.unwrap_or(0),
            3,
            "成功フィードから3件のリンクが保存されるべきです"
        );

        // 2. エラークライアントで全フィードを処理
        let error_client = MockHttpClient::new_error("接続タイムアウト");

        // エラークライアントでも処理自体は成功する（内部でエラーハンドリング）
        let all_result = task_collect_article_links(&error_client, &test_feeds, &pool).await;
        assert!(
            all_result.is_ok(),
            "エラーハンドリングが正しく動作していません"
        );

        // エラーフィードからは新たなリンクが追加されないことを確認
        let final_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            final_count.unwrap_or(0),
            3,
            "エラーフィードから新たなリンクが追加されるべきではありません"
        );

        // 3. 成功・エラー混在での処理確認
        // 新しいテーブル状態でテスト
        sqlx::query!("DELETE FROM article_links")
            .execute(&pool)
            .await?;

        // 混在処理では各フィードが個別に処理される
        // この関数は現在の実装ではクライアント固定なので、実際の混在テストは困難
        // その代わりに、成功ケースが正しく処理されることを再確認
        let final_result = task_collect_article_links(&success_client, &success_feeds, &pool).await;
        assert!(
            final_result.is_ok(),
            "最終的な成功フィード処理が失敗しました"
        );

        let final_success_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            final_success_count.unwrap_or(0),
            3,
            "最終的な成功フィード処理結果が不正です"
        );

        println!("✅ RSSエラーハンドリングテスト完了");
        println!("  エラーがあっても処理が継続されることを確認");

        Ok(())
    }

    #[sqlx::test]
    async fn test_task_collect_article_links_duplicate_handling(
        pool: PgPool,
    ) -> Result<(), anyhow::Error> {
        use crate::core::feed::Feed;
        use crate::infra::api::http::MockHttpClient;

        // 同一URLを持つ複数のフィードを準備（重複リンクを意図的に生成）
        let same_rss_url = "https://shared.example.com/common.xml";
        let duplicate_feeds = vec![
            Feed {
                group: "group1".to_string(),
                name: "shared_feed_1".to_string(),
                rss_link: same_rss_url.to_string(),
            },
            Feed {
                group: "group2".to_string(),
                name: "shared_feed_2".to_string(),
                rss_link: same_rss_url.to_string(),
            },
            Feed {
                group: "group3".to_string(),
                name: "shared_feed_3".to_string(),
                rss_link: same_rss_url.to_string(),
            },
        ];

        // MockHttpClientで成功レスポンスを設定
        let mock_client = MockHttpClient::new_success();

        // 初期状態の確認
        let initial_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            initial_count.unwrap_or(0),
            0,
            "初期状態でarticle_linksが空ではありません"
        );

        // 1回目の実行：最初のフィードを処理
        let first_feed = vec![duplicate_feeds[0].clone()];
        let result1 = task_collect_article_links(&mock_client, &first_feed, &pool).await;
        assert!(result1.is_ok(), "1回目のRSS収集処理が失敗しました");

        // 1回目実行後の件数確認（3件のリンクが挿入されるはず）
        let after_first_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            after_first_count.unwrap_or(0),
            3,
            "1回目実行後に3件のリンクが保存されるべきです"
        );

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
        let all_result = task_collect_article_links(&mock_client, &duplicate_feeds, &pool).await;
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
            let expected_link = format!("https://{}.example.com/{}", expected_hash, article_num);

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
        let unique_feed = vec![Feed {
            group: "unique".to_string(),
            name: "unique_feed".to_string(),
            rss_link: "https://unique.example.com/different.xml".to_string(),
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
