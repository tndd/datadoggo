use crate::infra::db::setup_database;
use crate::infra::db::DatabaseInsertResult;
use crate::infra::loader::load_file;
use anyhow::{Context, Result};
use rss::Channel;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

// RSS記事のリンク情報を格納する構造体（<item>要素のみ対象）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RssLink {
    pub link: String,
    pub title: String,
    pub pub_date: String,
}

// RSSのチャンネルから<item>要素のリンク情報を抽出する関数
pub fn extract_rss_links_from_channel(channel: &Channel) -> Vec<RssLink> {
    let mut rss_links = Vec::new();

    for item in channel.items() {
        if let (Some(link), Some(pub_date)) = (item.link(), item.pub_date()) {
            let rss_link = RssLink {
                link: link.to_string(),
                title: item.title().unwrap_or("タイトルなし").to_string(),
                pub_date: pub_date.to_string(),
            };
            rss_links.push(rss_link);
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
        return Ok(DatabaseInsertResxult::empty());
    }

    let mut tx = pool
        .begin()
        .await
        .context("トランザクションの開始に失敗しました")?;
    let mut total_inserted = 0;

    // sqlx::query!マクロを使用してコンパイル時にSQLを検証
    for link in rss_links {
        let result = sqlx::query!(
            r#"
            INSERT INTO rss_articles (link, title, pub_date)
            VALUES ($1, $2, $3)
            ON CONFLICT (link) DO NOTHING
            "#,
            link.link,
            link.title,
            link.pub_date
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
    pub pub_date_from: Option<String>,
    pub pub_date_to: Option<String>,
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

    let mut query = "SELECT link, title, pub_date FROM rss_articles".to_string();
    let mut conditions = Vec::new();
    let mut params = Vec::new();
    let mut param_count = 0;

    // linkによる絞り込み
    if let Some(link_contains) = &filter.link_contains {
        param_count += 1;
        conditions.push(format!("link ILIKE ${}", param_count));
        params.push(format!("%{}%", link_contains));
    }

    // pub_dateの範囲指定
    if let Some(pub_date_from) = &filter.pub_date_from {
        param_count += 1;
        conditions.push(format!("pub_date >= ${}", param_count));
        params.push(pub_date_from.clone());
    }

    if let Some(pub_date_to) = &filter.pub_date_to {
        param_count += 1;
        conditions.push(format!("pub_date <= ${}", param_count));
        params.push(pub_date_to.clone());
    }

    // WHERE句を追加
    if !conditions.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&conditions.join(" AND "));
    }

    // 日付順でソート
    query.push_str(" ORDER BY pub_date DESC");

    // 動的クエリを実行
    let mut query_builder = sqlx::query_as::<_, RssLink>(&query);

    for param in params {
        query_builder = query_builder.bind(param);
    }

    let rss_links = query_builder
        .fetch_all(pool)
        .await
        .context("RSSリンクの取得に失敗しました")?;

    Ok(rss_links)
}

/// 指定されたリンクのRSS記事を取得する
pub async fn get_rss_link_by_link(link: &str) -> Result<Option<RssLink>> {
    let pool = setup_database().await?;
    get_rss_link_by_link_with_pool(link, &pool).await
}

/// 指定されたリンクのRSSリンクを指定されたプールから取得する
pub async fn get_rss_link_by_link_with_pool(link: &str, pool: &PgPool) -> Result<Option<RssLink>> {
    let rss_link = sqlx::query_as::<_, RssLink>(
        "SELECT link, title, pub_date FROM rss_articles WHERE link = $1",
    )
    .bind(link)
    .fetch_optional(pool)
    .await
    .context("指定されたリンクのRSSリンク取得に失敗しました")?;

    Ok(rss_link)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    // XML解析関数のテスト
    mod xml_parsing_tests {
        use super::*;

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
                            <pubDate>Mon, 10 Aug 2025 12:00:00 +0000</pubDate>
                        </item>
                        <item>
                            <title>Test Article 2</title>
                            <link>http://example.com/article2</link>
                            <description>Test article 2 description</description>
                            <pubDate>Mon, 10 Aug 2025 13:00:00 +0000</pubDate>
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
                            <pubDate>Mon, 10 Aug 2025 14:00:00 +0000</pubDate>
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
                    pub_date: "2025-08-26T10:00:00Z".to_string(),
                },
                RssLink {
                    title: "Test Article 2".to_string(),
                    link: "https://test.example.com/article2".to_string(),
                    pub_date: "2025-08-26T11:00:00Z".to_string(),
                },
                RssLink {
                    title: "異なるドメイン記事".to_string(),
                    link: "https://different.domain.com/post".to_string(),
                    pub_date: "2025-08-26T12:00:00Z".to_string(),
                },
            ];

            // データベースに保存をテスト
            let result = save_rss_links_with_pool(&rss_basic, &pool).await?;

            // SaveResultの検証
            assert_eq!(result.inserted, 3, "新規挿入された記事数が期待と異なります");
            assert_eq!(
                result.skipped_duplicate, 0,
                "重複スキップ数が期待と異なります"
            );

            // 実際にデータベースに保存されたことを確認
            let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rss_articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, 3, "期待する件数(3件)が保存されませんでした");

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
                pub_date: "2025-08-26T13:00:00Z".to_string(),
            };

            // 重複記事を保存しようとする
            let result = save_rss_links_with_pool(&[duplicate_article], &pool).await?;

            // SaveResultの検証
            assert_eq!(
                result.inserted, 0,
                "重複記事が新規挿入されるべきではありません"
            );
            assert_eq!(
                result.skipped_duplicate, 1,
                "重複スキップ数が期待と異なります"
            );

            // データベースの件数は変わらない（19件のまま）
            let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rss_articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, 17, "重複記事が挿入され、件数が変わってしまいました");

            println!("✅ RSS重複スキップ検証成功: {}", result);

            Ok(())
        }

        #[sqlx::test]
        async fn test_empty_links(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
            let empty_articles: Vec<RssLink> = vec![];
            let result = save_rss_links_with_pool(&empty_articles, &pool).await?;

            // 空配列の結果検証
            assert_eq!(result.inserted, 0, "空配列の新規挿入数は0であるべきです");
            assert_eq!(
                result.skipped_duplicate, 0,
                "空配列の重複スキップ数は0であるべきです"
            );

            // データベースには何も挿入されていない
            let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rss_articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, 0, "空配列でもデータが挿入されてしまいました");

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
                    pub_date: "2025-08-26T14:00:00Z".to_string(),
                },
                RssLink {
                    title: "新規記事1".to_string(),
                    link: "https://test.example.com/new-article1".to_string(), // 新しいリンク
                    pub_date: "2025-08-26T15:00:00Z".to_string(),
                },
                RssLink {
                    title: "新規記事2".to_string(),
                    link: "https://another.domain.com/article".to_string(), // 異なるドメイン
                    pub_date: "2025-08-26T16:00:00Z".to_string(),
                },
            ];

            let result = save_rss_links_with_pool(&mixed_articles, &pool).await?;

            // SaveResultの検証
            assert_eq!(result.inserted, 2, "新規記事2件が挿入されるべきです");
            assert_eq!(
                result.skipped_duplicate, 1,
                "既存記事1件がスキップされるべきです"
            );

            // 最終的にデータベースには19件（fixture 17件 + 新規 2件）
            let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rss_articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, 19, "期待する件数(19件)と異なります");

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

            // 日付順（降順）でソートされていることを確認
            let mut prev_date: Option<&str> = None;
            for article in &articles {
                if let Some(prev) = prev_date {
                    assert!(
                        article.pub_date.as_str() <= prev,
                        "日付の降順ソートが正しくありません"
                    );
                }
                prev_date = Some(article.pub_date.as_str());
            }

            // 基本的な記事のフィールドがすべて設定されていることを確認
            for article in &articles {
                assert!(!article.title.is_empty(), "記事のタイトルが空です");
                assert!(!article.link.is_empty(), "記事のリンクが空です");
            }

            println!("✅ RSS全件取得際どいテスト成功: {}件", articles.len());

            Ok(())
        }

        #[sqlx::test(fixtures("rss"))]
        async fn test_date_boundary_edge_cases(
            pool: PgPool,
        ) -> Result<(), Box<dyn std::error::Error>> {
            // 2025-01-15の境界テスト

            // 開始時刻ちょうどを含む検索
            let filter_start = RssLinkFilter {
                pub_date_from: Some("2025-01-15T00:00:00Z".to_string()),
                pub_date_to: Some("2025-01-15T23:59:59Z".to_string()),
                ..Default::default()
            };

            let articles_boundary = get_rss_links_with_pool(Some(filter_start), &pool).await?;

            // 境界値の記事が含まれることを確認
            assert!(
                articles_boundary.len() >= 3,
                "2025-01-15の境界記事が期待通り取得されません"
            );

            let boundary_links: Vec<&str> =
                articles_boundary.iter().map(|a| a.link.as_str()).collect();
            assert!(boundary_links.contains(&"https://test.com/boundary/exactly-start"));
            assert!(boundary_links.contains(&"https://test.com/boundary/exactly-end"));
            assert!(boundary_links.contains(&"https://example.com/tech/article-2025-01-15"));

            // 1秒前後の記事は含まれないことを確認
            assert!(!boundary_links.contains(&"https://test.com/boundary/one-second-before"));
            assert!(!boundary_links.contains(&"https://test.com/boundary/one-second-after"));

            println!(
                "✅ RSS日付境界テスト成功: {}件の境界記事",
                articles_boundary.len()
            );

            Ok(())
        }

        #[sqlx::test(fixtures("rss"))]
        async fn test_get_rss_links_by_date_range(
            pool: PgPool,
        ) -> Result<(), Box<dyn std::error::Error>> {
            // 範囲の開始時刻ちょうどの記事が含まれるかテスト
            let filter_start_boundary = RssLinkFilter {
                pub_date_from: Some("2025-01-15T00:00:00Z".to_string()),
                pub_date_to: Some("2025-01-15T00:00:01Z".to_string()),
                ..Default::default()
            };

            let articles_start =
                get_rss_links_with_pool(Some(filter_start_boundary), &pool).await?;
            assert_eq!(
                articles_start.len(),
                1,
                "開始境界時刻ちょうどの記事が含まれていません"
            );
            assert_eq!(
                articles_start[0].link,
                "https://test.com/boundary/exactly-start"
            );

            // 範囲の終了時刻ちょうどの記事が含まれるかテスト
            let filter_end_boundary = RssLinkFilter {
                pub_date_from: Some("2025-01-15T23:59:58Z".to_string()),
                pub_date_to: Some("2025-01-15T23:59:59Z".to_string()),
                ..Default::default()
            };

            let articles_end = get_rss_links_with_pool(Some(filter_end_boundary), &pool).await?;
            assert_eq!(
                articles_end.len(),
                1,
                "終了境界時刻ちょうどの記事が含まれていません"
            );
            assert_eq!(
                articles_end[0].link,
                "https://test.com/boundary/exactly-end"
            );

            // 範囲外（1秒前）の記事が除外されるかテスト
            let filter_before = RssLinkFilter {
                pub_date_from: Some("2025-01-15T00:00:00Z".to_string()),
                pub_date_to: Some("2025-01-15T23:59:58Z".to_string()),
                ..Default::default()
            };

            let articles_before = get_rss_links_with_pool(Some(filter_before), &pool).await?;
            let before_links: Vec<&str> = articles_before.iter().map(|a| a.link.as_str()).collect();
            assert!(
                !before_links.contains(&"2025-01-14T23:59:59Z"),
                "範囲外の記事が含まれています"
            );

            println!("✅ RSS日付境界際どいテスト成功");

            Ok(())
        }

        #[sqlx::test(fixtures("rss"))]
        async fn test_get_rss_links_by_combined_filter(
            pool: PgPool,
        ) -> Result<(), Box<dyn std::error::Error>> {
            // 部分一致検索（example.comを含む）
            let filter_partial = RssLinkFilter {
                link_contains: Some("example.com".to_string()),
                ..Default::default()
            };

            let articles_partial = get_rss_links_with_pool(Some(filter_partial), &pool).await?;
            assert!(
                articles_partial.len() >= 4,
                "example.comを含む記事が少なすぎます"
            ); // 基本3件 + testing-advanced

            // 厳密でない一致（not-example.comは含まれるが確認）
            let example_count = articles_partial
                .iter()
                .filter(|a| a.link.contains("example.com") && !a.link.contains("not-example.com"))
                .count();
            let not_example_count = articles_partial
                .iter()
                .filter(|a| a.link.contains("not-example.com"))
                .count();

            assert!(
                example_count >= 4,
                "純粋なexample.comドメインの記事が足りません"
            );
            assert_eq!(
                not_example_count, 1,
                "not-example.comの記事も含まれるはずです"
            );

            // より具体的なパス部分の一致
            let filter_specific = RssLinkFilter {
                link_contains: Some("/tech/".to_string()),
                ..Default::default()
            };

            let articles_specific = get_rss_links_with_pool(Some(filter_specific), &pool).await?;
            assert_eq!(
                articles_specific.len(),
                1,
                "/tech/パスを含む記事は1件であるべきです"
            );
            assert_eq!(
                articles_specific[0].link,
                "https://example.com/tech/article-2025-01-15"
            );

            println!("✅ RSSURL部分一致詳細テスト成功");

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

            assert!(tech_article.is_some(), "指定された記事が見つかりません");
            let article = tech_article.unwrap();
            assert_eq!(article.title, "Tech News 2025");

            // 大小文字が異なるURLでの取得（完全一致のため失敗するはず）
            let case_different = get_rss_link_by_link_with_pool(
                "https://EXAMPLE.COM/tech/article-2025-01-15",
                &pool,
            )
            .await?;
            assert!(
                case_different.is_none(),
                "大小文字が異なるURLで記事が取得されました"
            );

            // 部分一致では取得できないことを確認
            let partial_match = get_rss_link_by_link_with_pool("example.com/tech", &pool).await?;
            assert!(partial_match.is_none(), "部分一致で記事が取得されました");

            // シンプルな記事取得テスト
            let simple_article =
                get_rss_link_by_link_with_pool("https://minimal.site.com/simple", &pool).await?;
            assert!(simple_article.is_some(), "シンプルな記事が取得できません");

            println!("✅ RSS個別記事取得際どいテスト成功");

            Ok(())
        }
    }

    // エッジケースとパフォーマンステスト
    mod edge_case_tests {
        use super::*;

        #[sqlx::test(fixtures("rss"))]
        async fn test_get_rss_links_no_match(
            pool: PgPool,
        ) -> Result<(), Box<dyn std::error::Error>> {
            // 大小文字を無視したILIKE検索のテスト
            let filter_case = RssLinkFilter {
                link_contains: Some("casesensitive".to_string()), // 小文字で検索
                ..Default::default()
            };

            let articles_case = get_rss_links_with_pool(Some(filter_case), &pool).await?;
            assert_eq!(articles_case.len(), 1, "大小文字無視検索が機能していません");
            assert_eq!(articles_case[0].link, "https://CaseSensitive.com/MixedCase");

            // 特殊文字を含むURL検索
            let filter_special = RssLinkFilter {
                link_contains: Some("%20with%20".to_string()),
                ..Default::default()
            };

            let articles_special = get_rss_links_with_pool(Some(filter_special), &pool).await?;
            assert_eq!(articles_special.len(), 1, "特殊文字検索が機能していません");

            // アンダースコア検索（SQLのワイルドカードではない）
            let filter_underscore = RssLinkFilter {
                link_contains: Some("article_with_underscore".to_string()), // より具体的な検索語
                ..Default::default()
            };

            let articles_underscore =
                get_rss_links_with_pool(Some(filter_underscore), &pool).await?;
            assert_eq!(
                articles_underscore.len(),
                1,
                "アンダースコア検索が機能していません"
            );
            assert!(
                articles_underscore[0]
                    .link
                    .contains("article_with_underscore"),
                "期待される記事がマッチしていません"
            );

            println!("✅ RSSリンク際どい絞り込みテスト成功");

            Ok(())
        }

        #[sqlx::test(fixtures("rss"))]
        async fn test_complex_combined_filters(
            pool: PgPool,
        ) -> Result<(), Box<dyn std::error::Error>> {
            // 非常に狭い時間範囲 + URL条件
            let filter_narrow = RssLinkFilter {
                link_contains: Some("example.com".to_string()),
                pub_date_from: Some("2025-01-15T09:00:00Z".to_string()),
                pub_date_to: Some("2025-01-15T11:00:00Z".to_string()),
            };

            let articles_narrow = get_rss_links_with_pool(Some(filter_narrow), &pool).await?;

            // Tech News記事のみがマッチするはず
            assert_eq!(articles_narrow.len(), 1, "狭い複合条件で1件が期待されます");
            assert_eq!(
                articles_narrow[0].link,
                "https://example.com/tech/article-2025-01-15"
            );

            // 存在しない組み合わせのテスト
            let filter_impossible = RssLinkFilter {
                link_contains: Some("example.com".to_string()),
                pub_date_from: Some("2025-01-20T00:00:00Z".to_string()),
                pub_date_to: Some("2025-01-25T00:00:00Z".to_string()),
            };

            let articles_impossible =
                get_rss_links_with_pool(Some(filter_impossible), &pool).await?;
            assert_eq!(
                articles_impossible.len(),
                0,
                "存在しない組み合わせでは0件が期待されます"
            );

            println!("✅ RSS複合条件際どいテスト成功");

            Ok(())
        }

        #[sqlx::test(fixtures("rss"))]
        async fn test_null_value_handling(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
            // pub_dateがNULLの記事は日付範囲検索に含まれないことを確認
            let filter_with_date = RssLinkFilter {
                pub_date_from: Some("2020-01-01T00:00:00Z".to_string()),
                ..Default::default()
            };

            let articles_with_date = get_rss_links_with_pool(Some(filter_with_date), &pool).await?;

            // すべての記事にpub_dateが設定されていることを確認（必須フィールド）
            for article in &articles_with_date {
                assert!(
                    !article.pub_date.is_empty(),
                    "pub_dateが空の記事が含まれています"
                );
            }

            // 個別記事取得でシンプルな記事が正常に取得できることを確認
            let simple_article =
                get_rss_link_by_link_with_pool("https://minimal.site.com/simple", &pool).await?;

            assert!(simple_article.is_some(), "シンプルな記事が取得できません");
            let article = simple_article.unwrap();
            assert_eq!(article.title, "シンプル記事");

            println!("✅ RSSNULL値処理テスト成功");

            Ok(())
        }

        #[sqlx::test(fixtures("rss"))]
        async fn test_performance_edge_cases(
            pool: PgPool,
        ) -> Result<(), Box<dyn std::error::Error>> {
            // 空文字列での検索
            let filter_empty = RssLinkFilter {
                link_contains: Some("".to_string()),
                ..Default::default()
            };

            let articles_empty = get_rss_links_with_pool(Some(filter_empty), &pool).await?;
            // 空文字列は全ての文字列を含むので、全記事が返される（NULLでない）
            assert!(
                articles_empty.len() > 0,
                "空文字列検索で記事が取得されません"
            );

            // 非常に長い検索文字列
            let long_string = "a".repeat(1000);
            let filter_long = RssLinkFilter {
                link_contains: Some(long_string),
                ..Default::default()
            };

            let articles_long = get_rss_links_with_pool(Some(filter_long), &pool).await?;
            assert_eq!(articles_long.len(), 0, "長い文字列検索で0件が期待されます");

            println!("✅ RSSパフォーマンステスト成功");

            Ok(())
        }
    }
}
