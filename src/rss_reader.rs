use rss::Channel;
use std::fs::File;
use std::io::BufReader;

// RSS記事の情報を格納する構造体
#[derive(Debug, Clone, serde::Serialize)]
pub struct RssArticle {
    pub title: String,
    pub link: String,
    pub description: Option<String>,
    pub pub_date: Option<String>,
}

// RSSのチャンネルから記事を抽出する関数
pub fn extract_rss_articles_from_channel(channel: &Channel) -> Vec<RssArticle> {
    let mut articles = Vec::new();

    for item in channel.items() {
        if let Some(link) = item.link() {
            let article = RssArticle {
                title: item.title().unwrap_or("タイトルなし").to_string(),
                link: link.to_string(),
                description: item.description().map(|d| d.to_string()),
                pub_date: item.pub_date().map(|d| d.to_string()),
            };
            articles.push(article);
        }
    }

    articles
}

// ファイルからRSSを読み込むヘルパー関数
pub fn read_channel_from_file(file_path: &str) -> Result<Channel, Box<dyn std::error::Error>> {
    let file = File::open(file_path)?;
    let buf_reader = BufReader::new(file);
    Channel::read_from(buf_reader).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // XMLからRSSチャンネルを解析するヘルパー関数
    fn parse_channel_from_xml(xml: &str) -> Result<Channel, Box<dyn std::error::Error>> {
        Channel::read_from(BufReader::new(Cursor::new(xml.as_bytes()))).map_err(Into::into)
    }

    // 記事の基本構造をチェックするヘルパー関数
    fn validate_articles(articles: &[RssArticle]) {
        for article in &articles[..3.min(articles.len())] {
            assert!(!article.title.is_empty(), "記事のタイトルが空です");
            assert!(!article.link.is_empty(), "記事のリンクが空です");
            assert!(
                article.link.starts_with("http"),
                "リンクがHTTP形式ではありません"
            );
        }
    }

    #[test]
    fn test_extract_rss_articles_from_xml() {
        // xml->channel->rss_articleの流れの確認
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
        let articles = extract_rss_articles_from_channel(&channel);

        assert_eq!(articles.len(), 2, "2件の記事が抽出されるはず");
        assert_eq!(articles[0].title, "Test Article 1");
        assert_eq!(articles[0].link, "http://example.com/article1");
        assert_eq!(articles[1].title, "Test Article 2");
        assert_eq!(articles[1].link, "http://example.com/article2");
    }

    #[test]
    fn test_extract_rss_articles_from_xml_missing_link() {
        // xml(リンク欠落)->channel->rss_articleの流れの確認
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
                    </item>
                </channel>
            </rss>
            "#;

        let channel = parse_channel_from_xml(xml_missing_link).expect("Failed to parse test RSS");
        let articles = extract_rss_articles_from_channel(&channel);

        assert_eq!(articles.len(), 1, "リンクがない記事は除外されるはず");
        assert_eq!(articles[0].title, "Article With Link");
    }

    #[test]
    fn test_extract_rss_articles_from_files() {
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
            let articles = extract_rss_articles_from_channel(&channel);
            assert!(!articles.is_empty(), "{}の記事が0件", feed_name);

            validate_articles(&articles);
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
