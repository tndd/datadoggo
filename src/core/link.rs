use crate::core::feed::Feed;
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
    pub url: String,
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
                url: link.to_string(),
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
        .fetch(&feed.rss_link, 30)
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
    let urls: Vec<String> = article_links.iter().map(|r| r.url.clone()).collect();
    let titles: Vec<String> = article_links.iter().map(|r| r.title.clone()).collect();
    let pub_dates: Vec<DateTime<Utc>> = article_links.iter().map(|r| r.pub_date).collect();
    let sources: Vec<String> = article_links.iter().map(|r| r.source.clone()).collect();

    // バルクUPSERT処理
    sqlx::query!(
        r#"
        INSERT INTO article_links (url, title, pub_date, source)
        SELECT * FROM UNNEST($1::text[], $2::text[], $3::timestamptz[], $4::text[])
        ON CONFLICT (url) DO UPDATE SET
            title = EXCLUDED.title,
            pub_date = EXCLUDED.pub_date,
            source = EXCLUDED.source
        WHERE (article_links.title, article_links.pub_date, article_links.source)
            IS DISTINCT FROM (EXCLUDED.title, EXCLUDED.pub_date, EXCLUDED.source)
        "#,
        &urls,
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
        SELECT url, title, pub_date, source
        FROM article_links
        WHERE
            ($1::text IS NULL OR url ILIKE '%' || $1 || '%')
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
pub async fn search_backlog_article_links(pool: &PgPool) -> Result<Vec<ArticleLink>> {
    let links = sqlx::query_as!(
        ArticleLink,
        r#"
        SELECT al.url, al.title, al.pub_date, al.source
        FROM article_links al
        LEFT JOIN articles a ON al.url = a.url
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
            assert!(!article_link.url.is_empty(), "記事のリンクが空です");
            assert!(
                article_link.url.starts_with("http"),
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

    // XML解析関数のテスト（関数名ベースに統一）
    mod get_article_links_from_channel {
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
            assert_eq!(article_links[0].url, "http://example.com/article1");
            assert_eq!(article_links[1].title, "Test Article 2");
            assert_eq!(article_links[1].url, "http://example.com/article2");
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

    // データベース保存機能のテスト（関数名ベースに統一）
    mod store_article_links {
        use super::*;

        #[sqlx::test]
        async fn test_save_links_to_db(pool: PgPool) -> Result<(), anyhow::Error> {
            // テスト用リンクデータを作成（必須フィールドのみ）
            let rss_basic = vec![
                ArticleLink {
                    title: "Test Article 1".to_string(),
                    url: "https://test.example.com/article1".to_string(),
                    pub_date: "2025-08-26T10:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
                ArticleLink {
                    title: "Test Article 2".to_string(),
                    url: "https://test.example.com/article2".to_string(),
                    pub_date: "2025-08-26T11:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
                ArticleLink {
                    title: "異なるドメイン記事".to_string(),
                    url: "https://different.domain.com/post".to_string(),
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

        #[sqlx::test(fixtures("rss"))]
        async fn test_duplicate_links(pool: PgPool) -> Result<(), anyhow::Error> {
            // fixtureで既に17件のデータが存在している状態

            // 同じリンクの記事を作成（重複）
            let duplicate_article_link = ArticleLink {
                title: "異なるタイトル".to_string(),
                url: "https://test.example.com/article1".to_string(), // fixtureと同じリンク
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

        #[sqlx::test(fixtures("rss"))]
        async fn test_mixed_new_and_existing_links(pool: PgPool) -> Result<(), anyhow::Error> {
            // fixtureで既に17件のデータが存在している状態

            // 1件は既存（重複）、2件は新規のデータを作成
            let mixed_articles = vec![
                ArticleLink {
                    title: "既存記事".to_string(),
                    url: "https://test.example.com/article1".to_string(), // fixtureと同じリンク
                    pub_date: "2025-08-26T14:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
                ArticleLink {
                    title: "新規記事1".to_string(),
                    url: "https://test.example.com/new-article1".to_string(), // 新しいリンク
                    pub_date: "2025-08-26T15:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
                ArticleLink {
                    title: "新規記事2".to_string(),
                    url: "https://another.domain.com/article".to_string(), // 異なるドメイン
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

    // HTTPクライアントを使用したフィード取得テスト（関数名ベースに統一）
    mod get_article_links_from_feed {
        use super::*;
        use crate::infra::api::http::MockHttpClient;

        #[tokio::test]
        async fn test_get_article_links_with_mock() -> Result<(), anyhow::Error> {
            // 動的XML生成を使用するモッククライアント
            let mock_client = MockHttpClient::new_success();

            let test_feed = Feed {
                group: "test".to_string(),
                name: "テストフィード".to_string(),
                rss_link: "https://example.com/rss.xml".to_string(),
            };

            let result = get_article_links_from_feed(&mock_client, &test_feed).await;

            assert!(result.is_ok(), "RSSフィードの取得が失敗");

            let article_links = result.unwrap();
            assert_eq!(article_links.len(), 3, "3件のリンクが取得されるべき"); // 動的XMLは3件の記事を生成

            // URLハッシュを計算
            use crate::infra::compute::generate_mock_rss_id;
            let hash = generate_mock_rss_id(&test_feed.rss_link);

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
                    link.url, expected_link,
                    "記事{}のリンクパターンが不正です",
                    article_num
                );
            }

            println!("✅ 動的XMLパターン検証完了 - ハッシュ: {}", hash);
            println!(
                "  記事1: {} -> {}",
                article_links[0].title, article_links[0].url
            );
            println!(
                "  記事2: {} -> {}",
                article_links[1].title, article_links[1].url
            );
            println!(
                "  記事3: {} -> {}",
                article_links[2].title, article_links[2].url
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
                rss_link: "https://example.com/error.xml".to_string(),
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

    // データベース取得機能のテスト（関数名ベースに統一）
    mod search_article_links {
        use super::*;

        #[sqlx::test(fixtures("rss"))]
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

        #[sqlx::test(fixtures("rss"))]
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
                article_links_start[0].url,
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
                article_links_end[0].url,
                "https://test.com/boundary/exactly-end"
            );

            // 1日全体の境界記事確認
            let filter_full_day = ArticleLinkQuery {
                link_pattern: None,
                pub_date_from: Some(parse_date("2025-01-15T00:00:00Z")?),
                pub_date_to: Some(parse_date("2025-01-15T23:59:59Z")?),
            };
            let article_links_day = search_article_links(Some(filter_full_day), &pool).await?;
            let day_links: Vec<&str> = article_links_day.iter().map(|a| a.url.as_str()).collect();
            assert!(day_links.contains(&"https://test.com/boundary/exactly-start"));
            assert!(day_links.contains(&"https://test.com/boundary/exactly-end"));
            assert!(day_links.contains(&"https://example.com/tech/article-2025-01-15"));
            assert!(!day_links.contains(&"https://test.com/boundary/one-second-before"));
            assert!(!day_links.contains(&"https://test.com/boundary/one-second-after"));

            println!("✅ RSS日付境界総合テスト成功");
            Ok(())
        }

        // ここに edge_cases 由来の3テストを統合
        #[sqlx::test(fixtures("rss_edge_cases"))]
        async fn test_search_article_links_boundary_conditions(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // 空文字タイトルの検索
            let empty_title_results = search_article_links(
                Some(ArticleLinkQuery {
                    link_pattern: Some("empty-title".to_string()),
                    pub_date_from: None,
                    pub_date_to: None,
                }),
                &pool,
            )
            .await?;

            assert_eq!(
                empty_title_results.len(),
                1,
                "空文字タイトル記事が見つかりませんでした"
            );
            assert_eq!(empty_title_results[0].title, "");

            // 非常に短いURL
            let short_results = search_article_links(
                Some(ArticleLinkQuery {
                    link_pattern: Some("x.co".to_string()),
                    pub_date_from: None,
                    pub_date_to: None,
                }),
                &pool,
            )
            .await?;
            assert_eq!(short_results.len(), 1, "最短URL記事が見つかりませんでした");
            assert_eq!(short_results[0].title, "最短URL記事");

            // 長いURL
            let long_url_results = search_article_links(
                Some(ArticleLinkQuery {
                    link_pattern: Some("very-long-subdomain".to_string()),
                    pub_date_from: None,
                    pub_date_to: None,
                }),
                &pool,
            )
            .await?;
            assert_eq!(
                long_url_results.len(),
                1,
                "長いURL記事が見つかりませんでした"
            );
            assert_eq!(long_url_results[0].title, "長いURL記事");

            Ok(())
        }

        #[sqlx::test(fixtures("rss_edge_cases"))]
        async fn test_date_precision_and_boundaries(pool: PgPool) -> Result<(), anyhow::Error> {
            // マイクロ秒精度の日付検索
            let precise_date_results = search_article_links(
                Some(ArticleLinkQuery {
                    link_pattern: Some("microsecond".to_string()),
                    pub_date_from: None,
                    pub_date_to: None,
                }),
                &pool,
            )
            .await?;
            assert_eq!(
                precise_date_results.len(),
                1,
                "マイクロ秒精度記事が見つかりませんでした"
            );

            // UNIX エポック境界
            let epoch_results = search_article_links(
                Some(ArticleLinkQuery {
                    link_pattern: None,
                    pub_date_from: Some(parse_date("1970-01-01T00:00:00Z")?),
                    pub_date_to: Some(parse_date("1970-01-01T00:00:02Z")?),
                }),
                &pool,
            )
            .await?;
            assert_eq!(
                epoch_results.len(),
                1,
                "UNIX エポック記事が見つかりませんでした"
            );
            assert_eq!(epoch_results[0].title, "UNIX開始日記事");

            // うるう年境界（2024-02-29）
            let leap_results = search_article_links(
                Some(ArticleLinkQuery {
                    link_pattern: None,
                    pub_date_from: Some(parse_date("2024-02-29T00:00:00Z")?),
                    pub_date_to: Some(parse_date("2024-02-29T23:59:59Z")?),
                }),
                &pool,
            )
            .await?;
            assert_eq!(leap_results.len(), 1, "うるう年記事が見つかりませんでした");
            assert_eq!(leap_results[0].title, "うるう年記事");
            Ok(())
        }

        #[sqlx::test(fixtures("rss_edge_cases"))]
        async fn test_case_insensitive_search(pool: PgPool) -> Result<(), anyhow::Error> {
            // 大文字小文字混在URLの検索（ILIKE動作確認）
            let case_results_lower = search_article_links(
                Some(ArticleLinkQuery {
                    link_pattern: Some("casesensitive".to_string()), // 小文字で検索
                    pub_date_from: None,
                    pub_date_to: None,
                }),
                &pool,
            )
            .await?;
            assert!(
                case_results_lower.len() >= 2,
                "大文字小文字を含むURL検索が正しく動作していません: {}件",
                case_results_lower.len()
            );
            let urls: Vec<&str> = case_results_lower
                .iter()
                .map(|link| link.url.as_str())
                .collect();
            assert!(urls.iter().any(|url| url.contains("CaseSensitive")));
            assert!(urls.iter().any(|url| url.contains("casesensitive")));
            Ok(())
        }
    }

    // バックログ取得のテストを独立モジュール化
    mod search_backlog_article_links {
        use super::*;

        #[sqlx::test(fixtures("rss_backlog"))]
        async fn test_search_backlog_article_links(pool: PgPool) -> Result<(), anyhow::Error> {
            let backlog_links = search_backlog_article_links(&pool).await?;
            assert_eq!(backlog_links.len(), 6);
            super::validate_date_sort_desc(&backlog_links);
            let links: Vec<&str> = backlog_links.iter().map(|l| l.url.as_str()).collect();
            assert!(links.contains(&"https://example.com/unprocessed-article-1"));
            assert!(links.contains(&"https://example.com/unprocessed-article-2"));
            assert!(links.contains(&"https://example.com/error-article-1"));
            assert!(links.contains(&"https://example.com/error-article-2"));
            assert!(links.contains(&"https://example.com/timeout-article"));
            assert!(links.contains(&"https://example.com/notfound-article"));
            Ok(())
        }

        #[sqlx::test]
        async fn test_search_backlog_article_links_empty(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            let backlog_links = search_backlog_article_links(&pool).await?;
            assert_eq!(backlog_links.len(), 0);
            Ok(())
        }
    }

    // store_article_links のエッジケース
    mod store_article_links_edge_cases {
        use super::*;

        #[sqlx::test(fixtures("rss_error_cases"))]
        async fn test_store_article_links_with_special_characters(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // 既存のfixture データを確認
            let initial_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;

            // 特殊文字を含む新しい記事リンクを追加
            let special_links = vec![
                ArticleLink {
                    title: "SQL インジェクションテスト'; DROP TABLE articles; --".to_string(),
                    url: "https://security-test.example.com/sql-injection-attempt".to_string(),
                    pub_date: "2025-01-21T10:00:00Z".parse().unwrap(),
                    source: "security-test".to_string(),
                },
                ArticleLink {
                    title: "XSS テスト <script>alert('xss')</script>".to_string(),
                    url: "https://security-test.example.com/xss-attempt".to_string(),
                    pub_date: "2025-01-21T11:00:00Z".parse().unwrap(),
                    source: "security-test".to_string(),
                },
            ];

            // 特殊文字を含む記事を保存
            let result = store_article_links(&special_links, &pool).await;
            assert!(
                result.is_ok(),
                "特殊文字を含む記事の保存が失敗しました: {:?}",
                result.err()
            );

            // 保存後の件数確認
            let final_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                final_count.unwrap_or(0),
                initial_count.unwrap_or(0) + 2,
                "特殊文字記事が正しく保存されませんでした"
            );

            // 実際に保存されたデータの確認
            let saved_article: Option<String> = sqlx::query_scalar!(
                "SELECT title FROM article_links WHERE url = $1",
                "https://security-test.example.com/sql-injection-attempt"
            )
            .fetch_optional(&pool)
            .await?;

            assert!(
                saved_article.is_some(),
                "SQL インジェクション対策記事が見つかりません"
            );
            assert!(
                saved_article
                    .unwrap()
                    .contains("'; DROP TABLE articles; --"),
                "特殊文字が正しくエスケープされて保存されていません"
            );

            println!("✅ 特殊文字・セキュリティテスト完了");
            Ok(())
        }

        #[sqlx::test(fixtures("rss_error_cases"))]
        async fn test_upsert_with_different_source_values(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // 同じURL、異なるsourceでの記事作成
            let duplicate_url = "https://duplicate-source.example.com/same-article";
            // Vecではなく固定長配列で十分なため、Clippyに従い配列に変更
            let links_with_different_sources = [
                ArticleLink {
                    title: "初回の記事".to_string(),
                    url: duplicate_url.to_string(),
                    pub_date: "2025-01-21T12:00:00Z".parse().unwrap(),
                    source: "first-source".to_string(),
                },
                ArticleLink {
                    title: "更新された記事".to_string(),
                    url: duplicate_url.to_string(),
                    pub_date: "2025-01-21T13:00:00Z".parse().unwrap(),
                    source: "second-source".to_string(),
                },
            ];

            // 1回目：初回保存
            store_article_links(&links_with_different_sources[0..1], &pool).await?;

            let first_save: (String, String) = sqlx::query_as!(
                ArticleLink,
                "SELECT url, title, pub_date, source FROM article_links WHERE url = $1",
                duplicate_url
            )
            .fetch_one(&pool)
            .await
            .map(|link| (link.title, link.source))?;

            assert_eq!(first_save.0, "初回の記事");
            assert_eq!(first_save.1, "first-source");

            // 2回目：更新（UPSERT）
            store_article_links(&links_with_different_sources[1..2], &pool).await?;

            let updated_save: (String, String) = sqlx::query_as!(
                ArticleLink,
                "SELECT url, title, pub_date, source FROM article_links WHERE url = $1",
                duplicate_url
            )
            .fetch_one(&pool)
            .await
            .map(|link| (link.title, link.source))?;

            assert_eq!(updated_save.0, "更新された記事");
            assert_eq!(updated_save.1, "second-source");

            // 同じURLの記事が1件のみ存在することを確認
            let count: i64 = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM article_links WHERE url = $1",
                duplicate_url
            )
            .fetch_one(&pool)
            .await?
            .unwrap_or(0);

            assert_eq!(count, 1, "同じURLの記事は1件のみであるべきです");

            println!("✅ 異なるsource値でのUPSERTテスト完了");
            Ok(())
        }
    }
}
