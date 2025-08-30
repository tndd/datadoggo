use crate::infra::db::setup_database;
use crate::infra::db::DatabaseInsertResult;
use crate::infra::loader::load_file;
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
    let mut rss_links = Vec::new();

    for item in channel.items() {
        if let (Some(link), Some(pub_date_str)) = (item.link(), item.pub_date()) {
            // RFC2822形式の日付文字列を解析
            if let Ok(parsed_date) = DateTime::parse_from_rfc2822(pub_date_str) {
                let rss_link = RssLink {
                    link: link.to_string(),
                    title: item.title().unwrap_or("タイトルなし").to_string(),
                    pub_date: parsed_date.with_timezone(&Utc),
                };
                rss_links.push(rss_link);
            }
        }
    }

    rss_links
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
pub async fn save_rss_links_to_db(articles: &[RssLink]) -> Result<DatabaseInsertResult> {
    let pool = setup_database().await?;
    save_rss_links_with_pool(articles, &pool).await
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
    pub link_contains: Option<String>,
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

    // 固定クエリを使用してsqlx::query!マクロでタイプセーフティを確保
    let rss_links = match (&filter.link_contains, &filter.pub_date_from, &filter.pub_date_to) {
        // フィルタなし
        (None, None, None) => {
            sqlx::query_as!(
                RssLink,
                "SELECT link, title, pub_date FROM rss_links ORDER BY pub_date DESC"
            )
            .fetch_all(pool)
            .await?
        }
        // リンクフィルタのみ
        (Some(link_pattern), None, None) => {
            let link_query = format!("%{}%", link_pattern);
            sqlx::query_as!(
                RssLink,
                "SELECT link, title, pub_date FROM rss_links WHERE link ILIKE $1 ORDER BY pub_date DESC",
                link_query
            )
            .fetch_all(pool)
            .await?
        }
        // 日付範囲フィルタのみ
        (None, Some(date_from), Some(date_to)) => {
            sqlx::query_as!(
                RssLink,
                "SELECT link, title, pub_date FROM rss_links WHERE pub_date >= $1 AND pub_date <= $2 ORDER BY pub_date DESC",
                date_from,
                date_to
            )
            .fetch_all(pool)
            .await?
        }
        // リンク + 日付範囲フィルタ
        (Some(link_pattern), Some(date_from), Some(date_to)) => {
            let link_query = format!("%{}%", link_pattern);
            sqlx::query_as!(
                RssLink,
                "SELECT link, title, pub_date FROM rss_links WHERE link ILIKE $1 AND pub_date >= $2 AND pub_date <= $3 ORDER BY pub_date DESC",
                link_query,
                date_from,
                date_to
            )
            .fetch_all(pool)
            .await?
        }
        // その他のパターンは簡易実装
        _ => {
            sqlx::query_as!(
                RssLink,
                "SELECT link, title, pub_date FROM rss_links ORDER BY pub_date DESC"
            )
            .fetch_all(pool)
            .await?
        }
    };

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
    fn validate_date_sort_desc(articles: &[RssLink]) {
        let mut prev_date: Option<DateTime<Utc>> = None;
        for article in articles {
            if let Some(prev) = prev_date {
                assert!(
                    article.pub_date <= prev,
                    "日付の降順ソートが正しくありません"
                );
            }
            prev_date = Some(article.pub_date);
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
            let articles = extract_rss_links_from_channel(&channel);

            assert_eq!(articles.len(), 2, "2件の記事が抽出されるはず");
            assert_eq!(articles[0].title, "Test Article 1");
            assert_eq!(articles[0].link, "http://example.com/article1");
            assert_eq!(articles[1].title, "Test Article 2");
            assert_eq!(articles[1].link, "http://example.com/article2");
        }

        #[test]
        fn test_extract_rss_links_from_xml_missing_link() {
            // xml(リンク欠落)->channel->rss_linkの流れの確認
            let xml_missing_link = r#"
                <rss version="2.0">
                    <channel>
                        <title>Test Feed</title>
                        <item>
                            <title>No Link Article</title>
                        </item>
                        <item>
                            <title>Article With Link</title>
                            <link>http://example.com/with-link</link>
                            <pubDate>Sun, 10 Aug 2025 14:00:00 +0000</pubDate>
                        </item>
                    </channel>
                </rss>
                "#;

            let channel =
                parse_channel_from_xml(xml_missing_link).expect("Failed to parse test RSS");
            let articles = extract_rss_links_from_channel(&channel);

            assert_eq!(
                articles.len(),
                1,
                "リンクまたはpub_dateがない記事は除外されるはず"
            );
            assert_eq!(articles[0].title, "Article With Link");
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
                let articles = extract_rss_links_from_channel(&channel);
                assert!(!articles.is_empty(), "{}の記事が0件", feed_name);

                validate_rss_links(&articles);
                println!("{}テスト結果: {}件の記事を抽出", feed_name, articles.len());
            }
        }

        #[test]
        fn test_read_non_existing_file() {
            // 存在しないファイルを読み込もうとするテスト
            let result = read_channel_from_file("non_existent_file.rss");
            assert!(result.is_err(), "存在しないファイルでエラーにならなかった");
        }
    }

    // データベース保存機能のテスト
    mod save_tests {
        use super::*;

        #[sqlx::test]
        async fn test_save_links_to_db(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
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
        async fn test_duplicate_links(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
            // fixtureで既に17件のデータが存在している状態

            // 同じリンクの記事を作成（重複）
            let duplicate_article = RssLink {
                title: "異なるタイトル".to_string(),
                link: "https://test.example.com/article1".to_string(), // fixtureと同じリンク
                pub_date: "2025-08-26T13:00:00Z".parse().unwrap(),
            };

            // 重複記事を保存しようとする
            let result = save_rss_links_with_pool(&[duplicate_article], &pool).await?;

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

        #[sqlx::test]
        async fn test_empty_links(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
            let empty_articles: Vec<RssLink> = vec![];
            let result = save_rss_links_with_pool(&empty_articles, &pool).await?;

            // 空配列の結果検証
            validate_save_result(&result, 0, 0);

            // データベースには何も挿入されていない
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM rss_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, Some(0), "空配列でもデータが挿入されてしまいました");

            println!("✅ RSS空配列処理検証成功: {}", result);

            Ok(())
        }

        #[sqlx::test(fixtures("rss"))]
        async fn test_mixed_new_and_existing_links(
            pool: PgPool,
        ) -> Result<(), Box<dyn std::error::Error>> {
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
        async fn test_get_all_rss_links_comprehensive(
            pool: PgPool,
        ) -> Result<(), Box<dyn std::error::Error>> {
            // 統合フィクスチャで19件のデータが存在

            let articles = get_rss_links_with_pool(None, &pool).await?;

            // 全件取得されることを確認
            assert!(articles.len() >= 17, "全件取得で最低17件が期待されます");

            // 基本的な検証（ソート順、フィールド存在）
            validate_date_sort_desc(&articles);
            validate_rss_links(&articles);

            println!("✅ RSS全件取得際どいテスト成功: {}件", articles.len());

            Ok(())
        }

        #[sqlx::test(fixtures("rss"))]
        async fn test_date_filtering_comprehensive(
            pool: PgPool,
        ) -> Result<(), Box<dyn std::error::Error>> {
            // 開始境界時刻の記事テスト
            let filter_start_boundary = RssLinkFilter {
                pub_date_from: Some("2025-01-15T00:00:00Z".parse().unwrap()),
                pub_date_to: Some("2025-01-15T00:00:01Z".parse().unwrap()),
                ..Default::default()
            };
            let articles_start =
                get_rss_links_with_pool(Some(filter_start_boundary), &pool).await?;
            assert_eq!(articles_start.len(), 1);
            assert_eq!(
                articles_start[0].link,
                "https://test.com/boundary/exactly-start"
            );

            // 終了境界時刻の記事テスト
            let filter_end_boundary = RssLinkFilter {
                pub_date_from: Some("2025-01-15T23:59:58Z".parse().unwrap()),
                pub_date_to: Some("2025-01-15T23:59:59Z".parse().unwrap()),
                ..Default::default()
            };
            let articles_end = get_rss_links_with_pool(Some(filter_end_boundary), &pool).await?;
            assert_eq!(articles_end.len(), 1);
            assert_eq!(
                articles_end[0].link,
                "https://test.com/boundary/exactly-end"
            );

            // 1日全体の境界記事確認
            let filter_full_day = RssLinkFilter {
                pub_date_from: Some("2025-01-15T00:00:00Z".parse().unwrap()),
                pub_date_to: Some("2025-01-15T23:59:59Z".parse().unwrap()),
                ..Default::default()
            };
            let articles_day = get_rss_links_with_pool(Some(filter_full_day), &pool).await?;
            let day_links: Vec<&str> = articles_day.iter().map(|a| a.link.as_str()).collect();
            assert!(day_links.contains(&"https://test.com/boundary/exactly-start"));
            assert!(day_links.contains(&"https://test.com/boundary/exactly-end"));
            assert!(day_links.contains(&"https://example.com/tech/article-2025-01-15"));
            assert!(!day_links.contains(&"https://test.com/boundary/one-second-before"));
            assert!(!day_links.contains(&"https://test.com/boundary/one-second-after"));

            println!("✅ RSS日付境界総合テスト成功");
            Ok(())
        }

        #[sqlx::test(fixtures("rss"))]
        async fn test_get_rss_links_by_combined_filter(
            pool: PgPool,
        ) -> Result<(), Box<dyn std::error::Error>> {
            // URL部分一致テスト
            let filter_partial = RssLinkFilter {
                link_contains: Some("example.com".to_string()),
                ..Default::default()
            };
            let articles_partial = get_rss_links_with_pool(Some(filter_partial), &pool).await?;
            assert!(articles_partial.len() >= 4);

            // 日付+URL複合条件テスト
            let filter_combined = RssLinkFilter {
                link_contains: Some("example.com".to_string()),
                pub_date_from: Some("2025-01-15T09:00:00Z".parse().unwrap()),
                pub_date_to: Some("2025-01-15T11:00:00Z".parse().unwrap()),
            };
            let articles_combined = get_rss_links_with_pool(Some(filter_combined), &pool).await?;
            assert_eq!(articles_combined.len(), 1);
            assert_eq!(
                articles_combined[0].link,
                "https://example.com/tech/article-2025-01-15"
            );

            println!("✅ RSS複合条件テスト成功");
            Ok(())
        }

        #[sqlx::test(fixtures("rss"))]
        async fn test_get_rss_link_by_link(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
            // 存在する記事の正確な取得
            let tech_article = get_rss_link_by_link_with_pool(
                "https://example.com/tech/article-2025-01-15",
                &pool,
            )
            .await?;
            assert!(tech_article.is_some());
            assert_eq!(tech_article.unwrap().title, "Tech News 2025");

            // 大小文字・部分一致では取得できない
            let case_different = get_rss_link_by_link_with_pool(
                "https://EXAMPLE.COM/tech/article-2025-01-15",
                &pool,
            )
            .await?;
            assert!(case_different.is_none());

            let partial_match = get_rss_link_by_link_with_pool("example.com/tech", &pool).await?;
            assert!(partial_match.is_none());

            println!("✅ RSS個別記事取得テスト成功");
            Ok(())
        }
    }

    // エッジケースとパフォーマンステスト
    mod edge_case_tests {
        use super::*;

        #[sqlx::test(fixtures("rss"))]
        async fn test_special_character_handling(
            pool: PgPool,
        ) -> Result<(), Box<dyn std::error::Error>> {
            // 大小文字無視検索
            let filter_case = RssLinkFilter {
                link_contains: Some("casesensitive".to_string()),
                ..Default::default()
            };
            let articles_case = get_rss_links_with_pool(Some(filter_case), &pool).await?;
            assert_eq!(articles_case.len(), 1);
            assert_eq!(articles_case[0].link, "https://CaseSensitive.com/MixedCase");

            // 特殊文字検索
            let filter_special = RssLinkFilter {
                link_contains: Some("%20with%20".to_string()),
                ..Default::default()
            };
            let articles_special = get_rss_links_with_pool(Some(filter_special), &pool).await?;
            assert_eq!(articles_special.len(), 1);

            // アンダースコア検索
            let filter_underscore = RssLinkFilter {
                link_contains: Some("article_with_underscore".to_string()),
                ..Default::default()
            };
            let articles_underscore =
                get_rss_links_with_pool(Some(filter_underscore), &pool).await?;
            assert_eq!(articles_underscore.len(), 1);

            println!("✅ RSS特殊文字処理テスト成功");
            Ok(())
        }

        #[sqlx::test(fixtures("rss"))]
        async fn test_filtering_edge_cases(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
            // 空文字列検索（全件取得）
            let filter_empty = RssLinkFilter {
                link_contains: Some("".to_string()),
                ..Default::default()
            };
            let articles_empty = get_rss_links_with_pool(Some(filter_empty), &pool).await?;
            assert!(articles_empty.len() > 0);

            // 長い検索文字列（0件）
            let filter_long = RssLinkFilter {
                link_contains: Some("a".repeat(1000)),
                ..Default::default()
            };
            let articles_long = get_rss_links_with_pool(Some(filter_long), &pool).await?;
            assert_eq!(articles_long.len(), 0);

            // 個別記事の正確な取得
            let simple_article =
                get_rss_link_by_link_with_pool("https://minimal.site.com/simple", &pool).await?;
            assert!(simple_article.is_some());
            assert_eq!(simple_article.unwrap().title, "シンプル記事");

            println!("✅ RSSフィルタリング境界テスト成功");
            Ok(())
        }
    }
}
