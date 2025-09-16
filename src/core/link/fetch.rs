use crate::core::link::model::ArticleLink;
use crate::core::rss::RssLink;
use crate::infra::api::http::HttpClient;
use crate::infra::parser::{parse_channel_from_xml_str, parse_date};
use anyhow::{Context, Result};
use rss::Channel;

/// rss_linkを用いてarticle_linkのリストを取得する
pub async fn fetch_article_links_using_feed<H: HttpClient>(
    client: &H,
    rss_link: &RssLink,
) -> Result<Vec<ArticleLink>> {
    let xml_content = client
        .fetch(&rss_link.url, 30)
        .await
        .context(format!("RSSフィードの取得に失敗: {}", rss_link))?;
    let channel = parse_channel_from_xml_str(&xml_content).context("XMLの解析に失敗")?;
    let article_links = get_article_links_from_channel(&channel);

    Ok(article_links)
}

// RSSのチャンネルから<item>要素のリンク情報を抽出する関数
fn get_article_links_from_channel(channel: &Channel) -> Vec<ArticleLink> {
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

    // HTTPクライアントを使用したフィード取得テスト（関数名ベースに統一）
    mod fetch_article_links_using_feed {
        use super::*;
        use crate::infra::api::http::MockHttpClient;

        #[tokio::test]
        async fn test_get_article_links_with_mock() -> Result<(), anyhow::Error> {
            // 動的XML生成を使用するモッククライアント
            let mock_client = MockHttpClient::new_success();

            let test_feed = RssLink {
                group: "test".to_string(),
                name: "テストフィード".to_string(),
                url: "https://example.com/rss.xml".to_string(),
            };

            let result = fetch_article_links_using_feed(&mock_client, &test_feed).await;

            assert!(result.is_ok(), "RSSフィードの取得が失敗");

            let article_links = result.unwrap();
            assert_eq!(article_links.len(), 3, "3件のリンクが取得されるべき"); // 動的XMLは3件の記事を生成

            // URLハッシュを計算
            use crate::infra::compute::generate_mock_rss_id;
            let hash = generate_mock_rss_id(&test_feed.url);

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

            let test_feed = RssLink {
                group: "test".to_string(),
                name: "エラーテストフィード".to_string(),
                url: "https://example.com/error.xml".to_string(),
            };

            let result = fetch_article_links_using_feed(&error_client, &test_feed).await;

            assert!(result.is_err(), "エラーが発生するべき");
            let error_msg = result.unwrap_err().to_string();
            println!("エラーメッセージ: {}", error_msg);
            // エラーが正しく伝播されていることを確認
            assert!(error_msg.contains("の取得に失敗"));

            println!("✅ HTTPモック使用のエラーハンドリングテスト完了");
            Ok(())
        }
    }
}
