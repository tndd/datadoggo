use crate::domain::feed::Feed;
use crate::infra::api::http::HttpClient;
use crate::infra::parser::{parse_channel_from_xml_str, parse_date};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rss::Channel;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

// 記事のリンク情報を格納する構造体（<item>要素のみ対象）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleLink {
    pub link: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub source: String,
}

// RSSのチャンネルから<item>要素のリンク情報を抽出する関数
pub fn get_article_links_from_channel(channel: &Channel) -> Vec<ArticleLink> {
    channel
        .items()
        .iter()
        .filter_map(|item| {
            let link = item.link()?;
            let pub_date_str = item.pub_date()?;
            let parsed_date = parse_date(pub_date_str).ok()?;

            Some(ArticleLink {
                link: link.to_string(),
                title: item.title().unwrap_or("タイトルなし").to_string(),
                pub_date: parsed_date,
                source: "rss".to_string(),
            })
        })
        .collect()
}

/// feedからarticle_linkのリストを取得する
pub async fn get_article_links_from_feed<H: HttpClient>(
    client: &H,
    feed: &Feed,
) -> Result<Vec<ArticleLink>> {
    let xml_content = client
        .fetch(&feed.article_link, 30)
        .await
        .context(format!("RSSフィードの取得に失敗: {}", feed))?;
    let channel = parse_channel_from_xml_str(&xml_content).context("XMLの解析に失敗")?;
    let article_links = get_article_links_from_channel(&channel);

    Ok(article_links)
}

/// # 概要
/// ArticleLinkの配列を指定されたデータベースプールに保存する。
///
/// # Note
/// sqlxの推奨パターンに従い、sqlx::query!マクロを使用してコンパイル時安全性を確保しています。
pub async fn store_article_links(article_links: &[ArticleLink], pool: &PgPool) -> Result<()> {
    if article_links.is_empty() {
        return Ok(());
    }

    // 配列として渡すためのデータ準備
    let links: Vec<String> = article_links.iter().map(|r| r.link.clone()).collect();
    let titles: Vec<String> = article_links.iter().map(|r| r.title.clone()).collect();
    let pub_dates: Vec<DateTime<Utc>> = article_links.iter().map(|r| r.pub_date).collect();
    let sources: Vec<String> = article_links.iter().map(|r| r.source.clone()).collect();

    // バルクUPSERT処理
    sqlx::query!(
        r#"
        INSERT INTO article_links (link, title, pub_date, source)
        SELECT * FROM UNNEST($1::text[], $2::text[], $3::timestamptz[], $4::text[])
        ON CONFLICT (link) DO UPDATE SET
            title = EXCLUDED.title,
            pub_date = EXCLUDED.pub_date,
            source = EXCLUDED.source
        WHERE (article_links.title, article_links.pub_date, article_links.source)
            IS DISTINCT FROM (EXCLUDED.title, EXCLUDED.pub_date, EXCLUDED.source)
        "#,
        &links,
        &titles,
        &pub_dates,
        &sources
    )
    .execute(pool)
    .await
    .context("記事リンクのバルクUPSERT処理に失敗しました")?;

    Ok(())
}

// 記事のフィルター条件を表す構造体
#[derive(Debug, Default)]
pub struct ArticleLinkQuery {
    pub link_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
}

/// # 概要
/// 指定されたデータベースプールから記事リンクを取得する。
pub async fn search_article_links(
    query: Option<ArticleLinkQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleLink>> {
    let query = query.unwrap_or_default();

    // 単一の静的SQL + オプション引数方式
    let article_links = sqlx::query_as!(
        ArticleLink,
        r#"
        SELECT link, title, pub_date, source
        FROM article_links
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

    Ok(article_links)
}

/// 未処理かエラーの記事リンクを取得する
pub async fn search_unprocessed_article_links(pool: &PgPool) -> Result<Vec<ArticleLink>> {
    let links = sqlx::query_as!(
        ArticleLink,
        r#"
        SELECT al.link, al.title, al.pub_date, al.source
        FROM article_links al
        LEFT JOIN articles a ON al.link = a.url
        WHERE a.url IS NULL OR a.status_code != 200
        ORDER BY al.pub_date DESC
        LIMIT 100
        "#
    )
    .fetch_all(pool)
    .await
    .context("未処理記事リンクの取得に失敗")?;

    Ok(links)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::parser::parse_channel_from_xml_str;
    use crate::infra::storage::file::load_channel_from_xml_file;

    // 記事の基本構造をチェックするヘルパー関数
    fn validate_article_links(article_links: &[ArticleLink]) {
        for article_link in &article_links[..3.min(article_links.len())] {
            assert!(!article_link.title.is_empty(), "記事のタイトルが空です");
            assert!(!article_link.link.is_empty(), "記事のリンクが空です");
            assert!(
                article_link.link.starts_with("http"),
                "リンクがHTTP形式ではありません"
            );
        }
    }

    // 日付ソートの検証ヘルパー関数
    fn validate_date_sort_desc(article_links: &[ArticleLink]) {
        let mut prev_date: Option<DateTime<Utc>> = None;
        for article_link in article_links {
            if let Some(prev) = prev_date {
                assert!(
                    article_link.pub_date <= prev,
                    "日付の降順ソートが正しくありません"
                );
            }
            prev_date = Some(article_link.pub_date);
        }
    }

    // XML解析関数のテスト
    mod xml_parsing_tests {
        use super::*;

        #[test]
        fn test_extract_article_links_from_xml() {
            // xml->channel->article_linkの流れの確認
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
            let article_links = get_article_links_from_channel(&channel);

            assert_eq!(article_links.len(), 2, "2件の記事が抽出されるはず");
            assert_eq!(article_links[0].title, "Test Article 1");
            assert_eq!(article_links[0].link, "http://example.com/article1");
            assert_eq!(article_links[1].title, "Test Article 2");
            assert_eq!(article_links[1].link, "http://example.com/article2");
        }

        #[test]
        fn test_extract_article_links_from_files() {
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
                let article_links = get_article_links_from_channel(&channel);
                assert!(!article_links.is_empty(), "{}の記事が0件", feed_name);

                validate_article_links(&article_links);
                println!(
                    "{}テスト結果: {}件の記事を抽出",
                    feed_name,
                    article_links.len()
                );
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
                ArticleLink {
                    title: "Test Article 1".to_string(),
                    link: "https://test.example.com/article1".to_string(),
                    pub_date: "2025-08-26T10:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
                ArticleLink {
                    title: "Test Article 2".to_string(),
                    link: "https://test.example.com/article2".to_string(),
                    pub_date: "2025-08-26T11:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
                ArticleLink {
                    title: "異なるドメイン記事".to_string(),
                    link: "https://different.domain.com/post".to_string(),
                    pub_date: "2025-08-26T12:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
            ];

            // データベースに保存をテスト
            store_article_links(&rss_basic, &pool).await?;

            // 実際にデータベースに保存されたことを確認
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, Some(3), "期待する件数(3件)が保存されませんでした");

            println!("✅ RSSリンク保存テスト成功: 3件");

            Ok(())
        }

        #[sqlx::test(fixtures("../../fixtures/rss.sql"))]
        async fn test_duplicate_links(pool: PgPool) -> Result<(), anyhow::Error> {
            // fixtureで既に17件のデータが存在している状態

            // 同じリンクの記事を作成（重複）
            let duplicate_article_link = ArticleLink {
                title: "異なるタイトル".to_string(),
                link: "https://test.example.com/article1".to_string(), // fixtureと同じリンク
                pub_date: "2025-08-26T13:00:00Z".parse().unwrap(),
                source: "test".to_string(),
            };

            // 重複記事を保存しようとする
            store_article_links(&[duplicate_article_link], &pool).await?;

            // データベースの件数は変わらない（19件のまま）
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                count,
                Some(17),
                "重複記事が挿入され、件数が変わってしまいました"
            );

            println!("✅ RSS重複スキップ検証成功");

            Ok(())
        }

        #[sqlx::test(fixtures("../../fixtures/rss.sql"))]
        async fn test_mixed_new_and_existing_links(pool: PgPool) -> Result<(), anyhow::Error> {
            // fixtureで既に17件のデータが存在している状態

            // 1件は既存（重複）、2件は新規のデータを作成
            let mixed_articles = vec![
                ArticleLink {
                    title: "既存記事".to_string(),
                    link: "https://test.example.com/article1".to_string(), // fixtureと同じリンク
                    pub_date: "2025-08-26T14:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
                ArticleLink {
                    title: "新規記事1".to_string(),
                    link: "https://test.example.com/new-article1".to_string(), // 新しいリンク
                    pub_date: "2025-08-26T15:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
                ArticleLink {
                    title: "新規記事2".to_string(),
                    link: "https://another.domain.com/article".to_string(), // 異なるドメイン
                    pub_date: "2025-08-26T16:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
            ];

            store_article_links(&mixed_articles, &pool).await?;

            // 最終的にデータベースには19件（fixture 17件 + 新規 2件）
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, Some(19), "期待する件数(19件)と異なります");

            println!("✅ RSS混在データ処理検証成功");

            Ok(())
        }
    }

    // HTTPクライアントを使用したフィード取得テスト
    mod feed_fetch_tests {
        use super::*;
        use crate::infra::api::http::MockHttpClient;

        #[tokio::test]
        async fn test_get_article_links_with_mock() -> Result<(), anyhow::Error> {
            // 動的XML生成を使用するモッククライアント
            let mock_client = MockHttpClient::new_success();

            let test_feed = Feed {
                group: "test".to_string(),
                name: "テストフィード".to_string(),
                article_link: "https://example.com/rss.xml".to_string(),
            };

            let result = get_article_links_from_feed(&mock_client, &test_feed).await;

            assert!(result.is_ok(), "RSSフィードの取得が失敗");

            let article_links = result.unwrap();
            assert_eq!(article_links.len(), 3, "3件のリンクが取得されるべき"); // 動的XMLは3件の記事を生成

            // URLハッシュを計算
            use crate::infra::compute::generate_mock_rss_id;
            let hash = generate_mock_rss_id(&test_feed.article_link);

            // 各記事の詳細検証
            for (index, link) in article_links.iter().enumerate() {
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
            println!(
                "  記事1: {} -> {}",
                article_links[0].title, article_links[0].link
            );
            println!(
                "  記事2: {} -> {}",
                article_links[1].title, article_links[1].link
            );
            println!(
                "  記事3: {} -> {}",
                article_links[2].title, article_links[2].link
            );

            println!("✅ HTTPモック使用のRSSフィード取得テスト完了");
            Ok(())
        }

        #[tokio::test]
        async fn test_get_article_links_with_error_mock() -> Result<(), anyhow::Error> {
            // エラーを返すモッククライアント
            let error_client = MockHttpClient::new_error("接続タイムアウト");

            let test_feed = Feed {
                group: "test".to_string(),
                name: "エラーテストフィード".to_string(),
                article_link: "https://example.com/error.xml".to_string(),
            };

            let result = get_article_links_from_feed(&error_client, &test_feed).await;

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
        async fn test_search_all_article_links_comprehensive(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // 統合フィクスチャで19件のデータが存在

            let article_links = search_article_links(None, &pool).await?;

            // 全件取得されることを確認
            assert!(
                article_links.len() >= 17,
                "全件取得で最低17件が期待されます"
            );

            // 基本的な検証（ソート順、フィールド存在）
            validate_date_sort_desc(&article_links);
            validate_article_links(&article_links);

            println!("✅ RSS全件取得際どいテスト成功: {}件", article_links.len());

            Ok(())
        }

        #[sqlx::test(fixtures("../../fixtures/rss.sql"))]
        async fn test_date_filtering_comprehensive(pool: PgPool) -> Result<(), anyhow::Error> {
            // 開始境界時刻の記事テスト
            let filter_start_boundary = ArticleLinkQuery {
                link_pattern: None,
                pub_date_from: Some(parse_date("2025-01-15T00:00:00Z")?),
                pub_date_to: Some(parse_date("2025-01-15T00:00:01Z")?),
            };
            let article_links_start =
                search_article_links(Some(filter_start_boundary), &pool).await?;
            assert_eq!(article_links_start.len(), 1);
            assert_eq!(
                article_links_start[0].link,
                "https://test.com/boundary/exactly-start"
            );

            // 終了境界時刻の記事テスト
            let filter_end_boundary = ArticleLinkQuery {
                link_pattern: None,
                pub_date_from: Some(parse_date("2025-01-15T23:59:58Z")?),
                pub_date_to: Some(parse_date("2025-01-15T23:59:59Z")?),
            };
            let article_links_end = search_article_links(Some(filter_end_boundary), &pool).await?;
            assert_eq!(article_links_end.len(), 1);
            assert_eq!(
                article_links_end[0].link,
                "https://test.com/boundary/exactly-end"
            );

            // 1日全体の境界記事確認
            let filter_full_day = ArticleLinkQuery {
                link_pattern: None,
                pub_date_from: Some(parse_date("2025-01-15T00:00:00Z")?),
                pub_date_to: Some(parse_date("2025-01-15T23:59:59Z")?),
            };
            let article_links_day = search_article_links(Some(filter_full_day), &pool).await?;
            let day_links: Vec<&str> = article_links_day.iter().map(|a| a.link.as_str()).collect();
            assert!(day_links.contains(&"https://test.com/boundary/exactly-start"));
            assert!(day_links.contains(&"https://test.com/boundary/exactly-end"));
            assert!(day_links.contains(&"https://example.com/tech/article-2025-01-15"));
            assert!(!day_links.contains(&"https://test.com/boundary/one-second-before"));
            assert!(!day_links.contains(&"https://test.com/boundary/one-second-after"));

            println!("✅ RSS日付境界総合テスト成功");
            Ok(())
        }

        #[sqlx::test(fixtures("../../fixtures/rss_backlog.sql"))]
        async fn test_search_backlog_article_links(pool: PgPool) -> Result<(), anyhow::Error> {
            // バックログのRSSリンクを取得
            let backlog_links = search_unprocessed_article_links(&pool).await?;

            // 未処理リンク2件 + エラーリンク4件 = 6件が返されることを確認
            assert_eq!(
                backlog_links.len(),
                6,
                "バックログRSSリンクの件数が期待値と異なります"
            );

            // 日付の降順ソートを確認
            validate_date_sort_desc(&backlog_links);

            // 各リンクの詳細確認
            let links: Vec<&str> = backlog_links.iter().map(|l| l.link.as_str()).collect();

            // 未処理リンクが含まれることを確認
            assert!(links.contains(&"https://example.com/unprocessed-article-1"));
            assert!(links.contains(&"https://example.com/unprocessed-article-2"));

            // エラーリンクが含まれることを確認
            assert!(links.contains(&"https://example.com/error-article-1"));
            assert!(links.contains(&"https://example.com/error-article-2"));
            assert!(links.contains(&"https://example.com/timeout-article"));
            assert!(links.contains(&"https://example.com/notfound-article"));

            // 正常処理済みリンクが含まれないことを確認
            assert!(!links.contains(&"https://example.com/success-article-1"));
            assert!(!links.contains(&"https://example.com/success-article-2"));

            println!(
                "✅ バックログRSSリンク取得テスト成功: {}件",
                backlog_links.len()
            );

            Ok(())
        }

        #[sqlx::test]
        async fn test_search_backlog_article_links_empty(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // 空のデータベースでテスト
            let backlog_links = search_unprocessed_article_links(&pool).await?;

            assert_eq!(
                backlog_links.len(),
                0,
                "空のデータベースでは0件が返されることを期待"
            );

            println!("✅ バックログRSSリンク空データベーステスト成功");

            Ok(())
        }
    }
}
