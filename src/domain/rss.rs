use crate::domain::feed::Feed;
use crate::infra::api::http::HttpClient;
use crate::infra::parser::{parse_channel_from_xml_str, parse_date};
use crate::infra::storage::db::InsertResult;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rss::Channel;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

// RSS記事のリンク情報を格納する構造体（<item>要素のみ対象）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RssLink {
    pub link: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
}

// RSSのチャンネルから<item>要素のリンク情報を抽出する関数
pub fn get_rss_links_from_channel(channel: &Channel) -> Vec<RssLink> {
    channel
        .items()
        .iter()
        .filter_map(|item| {
            let link = item.link()?;
            let pub_date_str = item.pub_date()?;
            let parsed_date = parse_date(pub_date_str).ok()?;

            Some(RssLink {
                link: link.to_string(),
                title: item.title().unwrap_or("タイトルなし").to_string(),
                pub_date: parsed_date,
            })
        })
        .collect()
}

/// feedからrss_linkのリストを取得する
pub async fn get_rss_links_from_feed<H: HttpClient>(
    client: &H,
    feed: &Feed,
) -> Result<Vec<RssLink>> {
    let xml_content = client
        .fetch(&feed.rss_link, 30)
        .await
        .context(format!("RSSフィードの取得に失敗: {}", feed))?;
    let channel = parse_channel_from_xml_str(&xml_content).context("XMLの解析に失敗")?;
    let rss_links = get_rss_links_from_channel(&channel);

    Ok(rss_links)
}

/// # 概要
/// RssLinkの配列を指定されたデータベースプールに保存する。
///
/// # Note
/// sqlxの推奨パターンに従い、sqlx::query!マクロを使用してコンパイル時安全性を確保しています。
pub async fn store_rss_links(rss_links: &[RssLink], pool: &PgPool) -> Result<InsertResult> {
    if rss_links.is_empty() {
        return Ok(InsertResult::empty());
    }

    let mut tx = pool
        .begin()
        .await
        .context("トランザクションの開始に失敗しました")?;
    let mut total_input = 0;

    // シンプルなUPSERT処理
    for rss_link in rss_links {
        let result = sqlx::query!(
            r#"
            INSERT INTO rss_links (link, title, pub_date)
            VALUES ($1, $2, $3)
            ON CONFLICT (link) DO UPDATE SET
                title = EXCLUDED.title,
                pub_date = EXCLUDED.pub_date
            WHERE rss_links.title IS DISTINCT FROM EXCLUDED.title
               OR rss_links.pub_date IS DISTINCT FROM EXCLUDED.pub_date
            "#,
            rss_link.link,
            rss_link.title,
            rss_link.pub_date
        )
        .execute(&mut *tx)
        .await
        .context("リンクのデータベースへの挿入・更新に失敗しました")?;

        if result.rows_affected() > 0 {
            total_input += 1;
        }
    }

    tx.commit()
        .await
        .context("トランザクションのコミットに失敗しました")?;

    let skipped = rss_links.len() - total_input;
    Ok(InsertResult::new(total_input, skipped))
}

// RSS記事のフィルター条件を表す構造体
#[derive(Debug, Default)]
pub struct RssLinkQuery {
    pub link_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
}

/// # 概要
/// 指定されたデータベースプールからRSSリンクを取得する。
pub async fn search_rss_links(query: Option<RssLinkQuery>, pool: &PgPool) -> Result<Vec<RssLink>> {
    let query = query.unwrap_or_default();

    // 単一の静的SQL + オプション引数方式
    let rss_links = sqlx::query_as!(
        RssLink,
        r#"
        SELECT link, title, pub_date
        FROM rss_links
        WHERE
            ($1::text IS NULL OR link ILIKE '%' || $1 || '%')
            AND ($2::timestamptz IS NULL OR pub_date >= $2)
            AND ($3::timestamptz IS NULL OR pub_date <= $3)
        ORDER BY pub_date DESC
        "#,
        query.link_pattern,
        query.pub_date_from,
        query.pub_date_to
    )
    .fetch_all(pool)
    .await?;

    Ok(rss_links)
}

/// 未処理のRSSリンクを取得する（articleテーブルに存在しないか、status_code != 200）
pub async fn search_unprocessed_rss_links(pool: &PgPool) -> Result<Vec<RssLink>> {
    let links = sqlx::query_as!(
        RssLink,
        r#"
        SELECT rl.link, rl.title, rl.pub_date
        FROM rss_links rl
        LEFT JOIN articles a ON rl.link = a.url
        WHERE a.url IS NULL OR a.status_code != 200
        ORDER BY rl.pub_date DESC
        LIMIT 100
        "#
    )
    .fetch_all(pool)
    .await
    .context("未処理RSSリンクの取得に失敗")?;

    Ok(links)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::parser::parse_channel_from_xml_str;
    use crate::infra::storage::file::load_channel_from_xml_file;

    // 記事の基本構造をチェックするヘルパー関数
    fn validate_rss_links(rss_links: &[RssLink]) {
        for rss_link in &rss_links[..3.min(rss_links.len())] {
            assert!(!rss_link.title.is_empty(), "記事のタイトルが空です");
            assert!(!rss_link.link.is_empty(), "記事のリンクが空です");
            assert!(
                rss_link.link.starts_with("http"),
                "リンクがHTTP形式ではありません"
            );
        }
    }

    // 日付ソートの検証ヘルパー関数
    fn validate_date_sort_desc(rss_links: &[RssLink]) {
        let mut prev_date: Option<DateTime<Utc>> = None;
        for rss_link in rss_links {
            if let Some(prev) = prev_date {
                assert!(
                    rss_link.pub_date <= prev,
                    "日付の降順ソートが正しくありません"
                );
            }
            prev_date = Some(rss_link.pub_date);
        }
    }

    // SaveResultの基本検証ヘルパー関数
    fn validate_save_result(result: &InsertResult, expected_input: usize, expected_skipped: usize) {
        assert_eq!(result.input, expected_input, "投入数が期待と異なります");
        assert_eq!(
            result.skipped, expected_skipped,
            "スキップ数が期待と異なります"
        );
    }

    // XML解析関数のテスト
    mod xml_parsing_tests {
        use super::*;

        #[test]
        fn test_extract_rss_links_from_xml() {
            // xml->channel->rss_linkの流れの確認
            let xml: &str = r#"
                <rss version="2.0">
                    <channel>
                        <title>Test Feed</title>
                        <link>http://example.com</link>
                        <description>Test Description</description>
                        <item>
                            <title>Test Article 1</title>
                            <link>http://example.com/article1</link>
                            <description>Test article 1 description</description>
                            <pubDate>Sun, 10 Aug 2025 12:00:00 +0000</pubDate>
                        </item>
                        <item>
                            <title>Test Article 2</title>
                            <link>http://example.com/article2</link>
                            <description>Test article 2 description</description>
                            <pubDate>Sun, 10 Aug 2025 13:00:00 +0000</pubDate>
                        </item>
                    </channel>
                </rss>
                "#;
            let channel = parse_channel_from_xml_str(xml).expect("Failed to parse test RSS");
            let rss_links = get_rss_links_from_channel(&channel);

            assert_eq!(rss_links.len(), 2, "2件の記事が抽出されるはず");
            assert_eq!(rss_links[0].title, "Test Article 1");
            assert_eq!(rss_links[0].link, "http://example.com/article1");
            assert_eq!(rss_links[1].title, "Test Article 2");
            assert_eq!(rss_links[1].link, "http://example.com/article2");
        }

        #[test]
        fn test_extract_rss_links_from_files() {
            // 複数の実際のRSSファイルからリンクを抽出するテスト
            let test_feeds = [
                ("mock/rss/bbc.rss", "BBC"),
                ("mock/rss/cbs.rss", "CBS"),
                ("mock/rss/guardian.rss", "Guardian"),
            ];

            for (file_path, feed_name) in &test_feeds {
                let result = load_channel_from_xml_file(file_path);
                assert!(result.is_ok(), "{}のRSSファイル読み込みに失敗", feed_name);

                let channel = result.unwrap();
                let rss_links = get_rss_links_from_channel(&channel);
                assert!(!rss_links.is_empty(), "{}の記事が0件", feed_name);

                validate_rss_links(&rss_links);
                println!("{}テスト結果: {}件の記事を抽出", feed_name, rss_links.len());
            }
        }
    }

    // データベース保存機能のテスト
    mod save_tests {
        use super::*;

        #[sqlx::test]
        async fn test_save_links_to_db(pool: PgPool) -> Result<(), anyhow::Error> {
            // テスト用リンクデータを作成（必須フィールドのみ）
            let rss_basic = vec![
                RssLink {
                    title: "Test Article 1".to_string(),
                    link: "https://test.example.com/article1".to_string(),
                    pub_date: "2025-08-26T10:00:00Z".parse().unwrap(),
                },
                RssLink {
                    title: "Test Article 2".to_string(),
                    link: "https://test.example.com/article2".to_string(),
                    pub_date: "2025-08-26T11:00:00Z".parse().unwrap(),
                },
                RssLink {
                    title: "異なるドメイン記事".to_string(),
                    link: "https://different.domain.com/post".to_string(),
                    pub_date: "2025-08-26T12:00:00Z".parse().unwrap(),
                },
            ];

            // データベースに保存をテスト
            let result = store_rss_links(&rss_basic, &pool).await?;

            // SaveResultの検証
            validate_save_result(&result, 3, 0);

            // 実際にデータベースに保存されたことを確認
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM rss_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, Some(3), "期待する件数(3件)が保存されませんでした");

            println!("✅ RSSリンク保存件数検証成功: {}件", result.input);
            println!(
                "✅ RSS SaveResult検証成功: {}",
                result.display_with_domain("RSSリンク")
            );

            Ok(())
        }

        #[sqlx::test(fixtures("../../fixtures/rss.sql"))]
        async fn test_duplicate_links(pool: PgPool) -> Result<(), anyhow::Error> {
            // fixtureで既に17件のデータが存在している状態

            // 同じリンクの記事を作成（重複）
            let duplicate_rss_link = RssLink {
                title: "異なるタイトル".to_string(),
                link: "https://test.example.com/article1".to_string(), // fixtureと同じリンク
                pub_date: "2025-08-26T13:00:00Z".parse().unwrap(),
            };

            // 重複記事を保存しようとする
            let result = store_rss_links(&[duplicate_rss_link], &pool).await?;

            // SaveResultの検証
            validate_save_result(&result, 1, 0);

            // データベースの件数は変わらない（19件のまま）
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM rss_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                count,
                Some(17),
                "重複記事が挿入され、件数が変わってしまいました"
            );

            println!("✅ RSS重複スキップ検証成功: {}", result);

            Ok(())
        }

        #[sqlx::test(fixtures("../../fixtures/rss.sql"))]
        async fn test_mixed_new_and_existing_links(pool: PgPool) -> Result<(), anyhow::Error> {
            // fixtureで既に17件のデータが存在している状態

            // 1件は既存（重複）、2件は新規のデータを作成
            let mixed_articles = vec![
                RssLink {
                    title: "既存記事".to_string(),
                    link: "https://test.example.com/article1".to_string(), // fixtureと同じリンク
                    pub_date: "2025-08-26T14:00:00Z".parse().unwrap(),
                },
                RssLink {
                    title: "新規記事1".to_string(),
                    link: "https://test.example.com/new-article1".to_string(), // 新しいリンク
                    pub_date: "2025-08-26T15:00:00Z".parse().unwrap(),
                },
                RssLink {
                    title: "新規記事2".to_string(),
                    link: "https://another.domain.com/article".to_string(), // 異なるドメイン
                    pub_date: "2025-08-26T16:00:00Z".parse().unwrap(),
                },
            ];

            let result = store_rss_links(&mixed_articles, &pool).await?;

            // SaveResultの検証
            validate_save_result(&result, 3, 0);

            // 最終的にデータベースには19件（fixture 17件 + 新規 2件）
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM rss_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, Some(19), "期待する件数(19件)と異なります");

            println!("✅ RSS混在データ処理検証成功: {}", result);

            Ok(())
        }
    }

    // HTTPクライアントを使用したフィード取得テスト
    mod feed_fetch_tests {
        use super::*;
        use crate::infra::api::http::MockHttpClient;

        #[tokio::test]
        async fn test_get_rss_links_with_mock() -> Result<(), anyhow::Error> {
            // 動的XML生成を使用するモッククライアント
            let mock_client = MockHttpClient::new_success();

            let test_feed = Feed {
                group: "test".to_string(),
                name: "テストフィード".to_string(),
                rss_link: "https://example.com/rss.xml".to_string(),
            };

            let result = get_rss_links_from_feed(&mock_client, &test_feed).await;

            assert!(result.is_ok(), "RSSフィードの取得が失敗");

            let rss_links = result.unwrap();
            assert_eq!(rss_links.len(), 3, "3件のリンクが取得されるべき"); // 動的XMLは3件の記事を生成

            // URLハッシュを計算
            use crate::infra::compute::generate_mock_rss_id;
            let hash = generate_mock_rss_id(&test_feed.rss_link);

            // 各記事の詳細検証
            for (index, link) in rss_links.iter().enumerate() {
                let article_num = index + 1;

                // タイトルのパターン検証 ("{hash}:title:{index}")
                let expected_title = format!("{}:title:{}", hash, article_num);
                assert_eq!(
                    link.title, expected_title,
                    "記事{}のタイトルパターンが不正です",
                    article_num
                );

                // リンクのパターン検証 ("https://{hash}.example.com/{index}")
                let expected_link = format!("https://{}.example.com/{}", hash, article_num);
                assert_eq!(
                    link.link, expected_link,
                    "記事{}のリンクパターンが不正です",
                    article_num
                );
            }

            println!("✅ 動的XMLパターン検証完了 - ハッシュ: {}", hash);
            println!("  記事1: {} -> {}", rss_links[0].title, rss_links[0].link);
            println!("  記事2: {} -> {}", rss_links[1].title, rss_links[1].link);
            println!("  記事3: {} -> {}", rss_links[2].title, rss_links[2].link);

            println!("✅ HTTPモック使用のRSSフィード取得テスト完了");
            Ok(())
        }

        #[tokio::test]
        async fn test_get_rss_links_with_error_mock() -> Result<(), anyhow::Error> {
            // エラーを返すモッククライアント
            let error_client = MockHttpClient::new_error("接続タイムアウト");

            let test_feed = Feed {
                group: "test".to_string(),
                name: "エラーテストフィード".to_string(),
                rss_link: "https://example.com/error.xml".to_string(),
            };

            let result = get_rss_links_from_feed(&error_client, &test_feed).await;

            assert!(result.is_err(), "エラーが発生するべき");
            let error_msg = result.unwrap_err().to_string();
            println!("エラーメッセージ: {}", error_msg);
            // エラーが正しく伝播されていることを確認
            assert!(error_msg.contains("の取得に失敗"));

            println!("✅ HTTPモック使用のエラーハンドリングテスト完了");
            Ok(())
        }
    }

    // データベース取得機能のテスト
    mod retrieval_tests {
        use super::*;

        #[sqlx::test(fixtures("../../fixtures/rss.sql"))]
        async fn test_search_all_rss_links_comprehensive(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // 統合フィクスチャで19件のデータが存在

            let rss_links = search_rss_links(None, &pool).await?;

            // 全件取得されることを確認
            assert!(rss_links.len() >= 17, "全件取得で最低17件が期待されます");

            // 基本的な検証（ソート順、フィールド存在）
            validate_date_sort_desc(&rss_links);
            validate_rss_links(&rss_links);

            println!("✅ RSS全件取得際どいテスト成功: {}件", rss_links.len());

            Ok(())
        }

        #[sqlx::test(fixtures("../../fixtures/rss.sql"))]
        async fn test_date_filtering_comprehensive(pool: PgPool) -> Result<(), anyhow::Error> {
            // 開始境界時刻の記事テスト
            let filter_start_boundary = RssLinkQuery {
                link_pattern: None,
                pub_date_from: Some(parse_date("2025-01-15T00:00:00Z")?),
                pub_date_to: Some(parse_date("2025-01-15T00:00:01Z")?),
            };
            let rss_links_start = search_rss_links(Some(filter_start_boundary), &pool).await?;
            assert_eq!(rss_links_start.len(), 1);
            assert_eq!(
                rss_links_start[0].link,
                "https://test.com/boundary/exactly-start"
            );

            // 終了境界時刻の記事テスト
            let filter_end_boundary = RssLinkQuery {
                link_pattern: None,
                pub_date_from: Some(parse_date("2025-01-15T23:59:58Z")?),
                pub_date_to: Some(parse_date("2025-01-15T23:59:59Z")?),
            };
            let rss_links_end = search_rss_links(Some(filter_end_boundary), &pool).await?;
            assert_eq!(rss_links_end.len(), 1);
            assert_eq!(
                rss_links_end[0].link,
                "https://test.com/boundary/exactly-end"
            );

            // 1日全体の境界記事確認
            let filter_full_day = RssLinkQuery {
                link_pattern: None,
                pub_date_from: Some(parse_date("2025-01-15T00:00:00Z")?),
                pub_date_to: Some(parse_date("2025-01-15T23:59:59Z")?),
            };
            let rss_links_day = search_rss_links(Some(filter_full_day), &pool).await?;
            let day_links: Vec<&str> = rss_links_day.iter().map(|a| a.link.as_str()).collect();
            assert!(day_links.contains(&"https://test.com/boundary/exactly-start"));
            assert!(day_links.contains(&"https://test.com/boundary/exactly-end"));
            assert!(day_links.contains(&"https://example.com/tech/article-2025-01-15"));
            assert!(!day_links.contains(&"https://test.com/boundary/one-second-before"));
            assert!(!day_links.contains(&"https://test.com/boundary/one-second-after"));

            println!("✅ RSS日付境界総合テスト成功");
            Ok(())
        }
    }
}
