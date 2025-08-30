use crate::infra::db::setup_database;
use crate::infra::db::DatabaseInsertResult;
use crate::infra::loader::load_file;
use crate::infra::parser::parse_date;
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
pub fn extract_rss_links_from_channel(channel: &Channel) -> Vec<RssLink> {
    channel
        .items()
        .iter()
        .filter_map(|item| {
            if let (Some(link), Some(pub_date_str)) = (item.link(), item.pub_date()) {
                // infra::parserを利用して日付文字列を解析
                if let Ok(parsed_date) = parse_date(pub_date_str) {
                    let rss_link = RssLink {
                        link: link.to_string(),
                        title: item.title().unwrap_or("タイトルなし").to_string(),
                        pub_date: parsed_date, // 既にUTC
                    };
                    Some(rss_link)
                } else {
                    None // 日付の解析に失敗した場合はスキップ
                }
            } else {
                None // リンクまたはpub_dateがない場合はスキップ
            }
        })
        .collect()
}

// ファイルからRSSを読み込むヘルパー関数（loaderを使用）
pub fn read_channel_from_file(file_path: &str) -> Result<Channel> {
    let buf_reader = load_file(file_path)?;
    Channel::read_from(buf_reader)
        .with_context(|| format!("RSSファイルの解析に失敗: {}", file_path))
}

/// # 概要
/// RssLinkの配列をデータベースに保存する。
///
/// ## 動作
/// - 自動でデータベース接続プールを作成
/// - マイグレーションを実行
/// - RSS記事を一括保存
/// - 重複記事は保存をスキップ
///
/// ## 引数
/// - `articles`: 保存するRSS記事のスライス
///
/// ## 戻り値
/// - `DatabaseInsertResult`: 保存件数の詳細
///
/// ## エラー
/// 操作失敗時には全ての操作をロールバックする。
pub async fn save_rss_links_to_db(rss_links: &[RssLink]) -> Result<DatabaseInsertResult> {
    let pool = setup_database().await?;
    save_rss_links_with_pool(rss_links, &pool).await
}

/// # 概要
/// RssLinkの配列を指定されたデータベースプールに保存する。
/// 既にプールを準備している場合は `save_rss_links_to_db` ではなく、この関数を使用する。
///
/// # Note
/// sqlxの推奨パターンに従い、sqlx::query!マクロを使用してコンパイル時安全性を確保しています。
pub async fn save_rss_links_with_pool(
    rss_links: &[RssLink],
    pool: &PgPool,
) -> Result<DatabaseInsertResult> {
    if rss_links.is_empty() {
        return Ok(DatabaseInsertResult::empty());
    }

    let mut tx = pool
        .begin()
        .await
        .context("トランザクションの開始に失敗しました")?;
    let mut total_inserted = 0;

    // sqlx::query!マクロを使用してコンパイル時にSQLを検証
    for rss_link in rss_links {
        let result = sqlx::query!(
            r#"
            INSERT INTO rss_links (link, title, pub_date)
            VALUES ($1, $2, $3)
            ON CONFLICT (link) DO NOTHING
            "#,
            rss_link.link,
            rss_link.title,
            rss_link.pub_date
        )
        .execute(&mut *tx)
        .await
        .context("リンクのデータベースへの挿入に失敗しました")?;

        if result.rows_affected() > 0 {
            total_inserted += 1;
        }
    }

    tx.commit()
        .await
        .context("トランザクションのコミットに失敗しました")?;

    Ok(DatabaseInsertResult::new(
        total_inserted,
        rss_links.len() - total_inserted,
    ))
}

// RSS記事のフィルター条件を表す構造体
#[derive(Debug, Default)]
pub struct RssLinkFilter {
    pub link_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
}

/// # 概要
/// データベースからRSS記事を取得する。
///
/// ## 動作
/// - 自動でデータベース接続プールを作成
/// - 指定された条件でRSS記事を取得
///
/// ## 引数
/// - `filter`: フィルター条件。Noneの場合は全件取得
///
/// ## 戻り値
/// - `Vec<RssLink>`: 条件にマッチしたRSS記事のリスト
pub async fn get_rss_links_from_db(filter: Option<RssLinkFilter>) -> Result<Vec<RssLink>> {
    let pool = setup_database().await?;
    get_rss_links_with_pool(filter, &pool).await
}

/// # 概要
/// 指定されたデータベースプールからRSSリンクを取得する。
pub async fn get_rss_links_with_pool(
    filter: Option<RssLinkFilter>,
    pool: &PgPool,
) -> Result<Vec<RssLink>> {
    let filter = filter.unwrap_or_default();

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
        filter.link_pattern,
        filter.pub_date_from,
        filter.pub_date_to
    )
    .fetch_all(pool)
    .await?;

    Ok(rss_links)
}

/// 指定されたリンクのRSS記事を取得する
pub async fn get_rss_link_by_link(link: &str) -> Result<Option<RssLink>> {
    let pool = setup_database().await?;
    get_rss_link_by_link_with_pool(link, &pool).await
}

/// 指定されたリンクのRSSリンクを指定されたプールから取得する
pub async fn get_rss_link_by_link_with_pool(link: &str, pool: &PgPool) -> Result<Option<RssLink>> {
    let rss_link = sqlx::query_as!(
        RssLink,
        "SELECT link, title, pub_date FROM rss_links WHERE link = $1",
        link
    )
    .fetch_optional(pool)
    .await
    .context("指定されたリンクのRSSリンク取得に失敗しました")?;

    Ok(rss_link)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    // 共通テストヘルパー関数
    // XMLからRSSチャンネルを解析するヘルパー関数
    fn parse_channel_from_xml(xml: &str) -> Result<Channel> {
        Channel::read_from(BufReader::new(Cursor::new(xml.as_bytes())))
            .context("XMLからのRSSチャンネル解析に失敗")
    }

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
    fn validate_save_result(
        result: &DatabaseInsertResult,
        expected_inserted: usize,
        expected_skipped: usize,
    ) {
        assert_eq!(
            result.inserted, expected_inserted,
            "新規挿入数が期待と異なります"
        );
        assert_eq!(
            result.skipped_duplicate, expected_skipped,
            "重複スキップ数が期待と異なります"
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
            let channel = parse_channel_from_xml(xml).expect("Failed to parse test RSS");
            let rss_links = extract_rss_links_from_channel(&channel);

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
                let result = read_channel_from_file(file_path);
                assert!(result.is_ok(), "{}のRSSファイル読み込みに失敗", feed_name);

                let channel = result.unwrap();
                let rss_links = extract_rss_links_from_channel(&channel);
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
            let result = save_rss_links_with_pool(&rss_basic, &pool).await?;

            // SaveResultの検証
            validate_save_result(&result, 3, 0);

            // 実際にデータベースに保存されたことを確認
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM rss_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, Some(3), "期待する件数(3件)が保存されませんでした");

            println!("✅ RSSリンク保存件数検証成功: {}件", result.inserted);
            println!(
                "✅ RSS SaveResult検証成功: {}",
                result.display_with_domain("RSSリンク")
            );

            Ok(())
        }

        #[sqlx::test(fixtures("rss"))]
        async fn test_duplicate_links(pool: PgPool) -> Result<(), anyhow::Error> {
            // fixtureで既に17件のデータが存在している状態

            // 同じリンクの記事を作成（重複）
            let duplicate_rss_link = RssLink {
                title: "異なるタイトル".to_string(),
                link: "https://test.example.com/article1".to_string(), // fixtureと同じリンク
                pub_date: "2025-08-26T13:00:00Z".parse().unwrap(),
            };

            // 重複記事を保存しようとする
            let result = save_rss_links_with_pool(&[duplicate_rss_link], &pool).await?;

            // SaveResultの検証
            validate_save_result(&result, 0, 1);

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

        #[sqlx::test(fixtures("rss"))]
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

            let result = save_rss_links_with_pool(&mixed_articles, &pool).await?;

            // SaveResultの検証
            validate_save_result(&result, 2, 1);

            // 最終的にデータベースには19件（fixture 17件 + 新規 2件）
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM rss_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, Some(19), "期待する件数(19件)と異なります");

            println!("✅ RSS混在データ処理検証成功: {}", result);

            Ok(())
        }
    }

    // データベース取得機能のテスト
    mod retrieval_tests {
        use super::*;

        #[sqlx::test(fixtures("rss"))]
        async fn test_get_all_rss_links_comprehensive(pool: PgPool) -> Result<(), anyhow::Error> {
            // 統合フィクスチャで19件のデータが存在

            let rss_links = get_rss_links_with_pool(None, &pool).await?;

            // 全件取得されることを確認
            assert!(rss_links.len() >= 17, "全件取得で最低17件が期待されます");

            // 基本的な検証（ソート順、フィールド存在）
            validate_date_sort_desc(&rss_links);
            validate_rss_links(&rss_links);

            println!("✅ RSS全件取得際どいテスト成功: {}件", rss_links.len());

            Ok(())
        }

        #[sqlx::test(fixtures("rss"))]
        async fn test_date_filtering_comprehensive(pool: PgPool) -> Result<(), anyhow::Error> {
            // 開始境界時刻の記事テスト
            let filter_start_boundary = RssLinkFilter {
                link_pattern: None,
                pub_date_from: Some(parse_date("2025-01-15T00:00:00Z")?),
                pub_date_to: Some(parse_date("2025-01-15T00:00:01Z")?),
            };
            let rss_links_start =
                get_rss_links_with_pool(Some(filter_start_boundary), &pool).await?;
            assert_eq!(rss_links_start.len(), 1);
            assert_eq!(
                rss_links_start[0].link,
                "https://test.com/boundary/exactly-start"
            );

            // 終了境界時刻の記事テスト
            let filter_end_boundary = RssLinkFilter {
                link_pattern: None,
                pub_date_from: Some(parse_date("2025-01-15T23:59:58Z")?),
                pub_date_to: Some(parse_date("2025-01-15T23:59:59Z")?),
            };
            let rss_links_end = get_rss_links_with_pool(Some(filter_end_boundary), &pool).await?;
            assert_eq!(rss_links_end.len(), 1);
            assert_eq!(
                rss_links_end[0].link,
                "https://test.com/boundary/exactly-end"
            );

            // 1日全体の境界記事確認
            let filter_full_day = RssLinkFilter {
                link_pattern: None,
                pub_date_from: Some(parse_date("2025-01-15T00:00:00Z")?),
                pub_date_to: Some(parse_date("2025-01-15T23:59:59Z")?),
            };
            let rss_links_day = get_rss_links_with_pool(Some(filter_full_day), &pool).await?;
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
