use rss::Channel;
use std::fs::File;
use std::io::{BufReader, Cursor};

// RSS記事の情報を格納する構造体
#[derive(Debug, Clone)]
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

// XMLからRSSチャンネルを解析するヘルパー関数
pub fn parse_channel_from_xml(xml: &str) -> Result<Channel, Box<dyn std::error::Error>> {
    Channel::read_from(BufReader::new(Cursor::new(xml.as_bytes()))).map_err(Into::into)
}

fn main() {
    match read_channel_from_file("mock/rss/bbc.rss") {
        Ok(channel) => {
            let articles = extract_rss_articles_from_channel(&channel);
            println!("BBCのRSSから{}件の記事を抽出しました:", articles.len());
            for (i, article) in articles.iter().enumerate() {
                println!("{}. {}", i + 1, article.title);
                println!("   リンク: {}", article.link);
                if let Some(desc) = &article.description {
                    println!("   説明: {}", desc);
                }
                println!();
            }
        }
        Err(e) => {
            eprintln!("エラーが発生しました: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rss::Channel;

    // テスト用のヘルパー関数: XMLからChannelオブジェクトを作成
    fn create_test_channel(xml: &str) -> Channel {
        parse_channel_from_xml(xml).expect("Failed to create test channel")
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
    fn test_extract_links_from_rss() {
        let test_rss: &str = r#"
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
        let channel = create_test_channel(test_rss);
        let articles = extract_rss_articles_from_channel(&channel);

        assert_eq!(articles.len(), 2, "2件の記事が抽出されるはず");
        assert_eq!(articles[0].title, "Test Article 1");
        assert_eq!(articles[0].link, "http://example.com/article1");
        assert_eq!(articles[1].title, "Test Article 2");
        assert_eq!(articles[1].link, "http://example.com/article2");
    }

    #[test]
    fn test_extract_links_from_missing_link_rss() {
        let test_rss = r#"
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

        let channel = create_test_channel(test_rss);
        let articles = extract_rss_articles_from_channel(&channel);

        assert_eq!(articles.len(), 1, "リンクがない記事は除外されるはず");
        assert_eq!(articles[0].title, "Article With Link");
    }

    #[test]
    fn test_extract_links_from_bbc_rss() {
        // BBC RSSファイルからリンクを抽出するテスト
        let result = read_channel_from_file("mock/rss/bbc.rss");

        assert!(result.is_ok(), "RSSファイルの読み込みに失敗しました");

        let channel = result.unwrap();
        let articles = extract_rss_articles_from_channel(&channel);
        assert!(!articles.is_empty(), "抽出された記事が0件でした");

        // 最初の記事をチェック
        let first_article = &articles[0];
        assert!(!first_article.title.is_empty(), "記事のタイトルが空です");
        assert!(!first_article.link.is_empty(), "記事のリンクが空です");
        assert!(
            first_article.link.starts_with("http"),
            "リンクがHTTP形式ではありません"
        );

        println!("テスト結果: {}件の記事を正常に抽出しました", articles.len());
    }

    #[test]
    fn test_extract_links_from_cbs_rss() {
        // CBS RSSファイルからリンクを抽出するテスト
        let result = read_channel_from_file("mock/rss/cbs.rss");

        assert!(result.is_ok(), "CBS RSSファイルの読み込みに失敗しました");

        let channel = result.unwrap();
        let articles = extract_rss_articles_from_channel(&channel);
        assert!(!articles.is_empty(), "抽出された記事が0件でした");

        // 記事の構造をチェック
        validate_articles(&articles);

        println!(
            "CBSテスト結果: {}件の記事を正常に抽出しました",
            articles.len()
        );
    }

    #[test]
    fn test_extract_links_from_guardian_rss() {
        // Guardian RSSファイルからリンクを抽出するテスト
        let result = read_channel_from_file("mock/rss/guardian.rss");

        assert!(
            result.is_ok(),
            "Guardian RSSファイルの読み込みに失敗しました"
        );

        let channel = result.unwrap();
        let articles = extract_rss_articles_from_channel(&channel);
        assert!(!articles.is_empty(), "抽出された記事が0件でした");

        // 記事の構造をチェック
        validate_articles(&articles);

        println!(
            "Guardianテスト結果: {}件の記事を正常に抽出しました",
            articles.len()
        );
    }

    #[test]
    fn test_invalid_file_path() {
        // 存在しないファイルのテスト
        let result = read_channel_from_file("non_existent_file.rss");
        assert!(result.is_err(), "存在しないファイルでエラーにならなかった");
    }

    #[test]
    fn test_rss_article_structure() {
        // RssArticle構造体のテスト
        let article = RssArticle {
            title: "テスト記事".to_string(),
            link: "https://example.com/test".to_string(),
            description: Some("テスト説明".to_string()),
            pub_date: Some("2025-07-27".to_string()),
        };

        assert_eq!(article.title, "テスト記事");
        assert_eq!(article.link, "https://example.com/test");
        assert_eq!(article.description, Some("テスト説明".to_string()));
        assert_eq!(article.pub_date, Some("2025-07-27".to_string()));
    }
}
