use rss::Channel;
use std::fs::File;
use std::io::BufReader;

// RSS記事の情報を格納する構造体
#[derive(Debug, Clone)]
pub struct RssArticle {
    pub title: String,
    pub link: String,
    pub description: Option<String>,
    pub pub_date: Option<String>,
}

// RSSファイルからリンクを抽出する関数
pub fn extract_links_from_rss(
    file_path: &str,
) -> Result<Vec<RssArticle>, Box<dyn std::error::Error>> {
    let file = File::open(file_path)?;
    let buf_reader = BufReader::new(file);
    let channel = Channel::read_from(buf_reader)?;

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

    Ok(articles)
}

fn main() {
    match extract_links_from_rss("mock/rss/bbc.rss") {
        Ok(articles) => {
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

    #[test]
    fn test_extract_links_from_bbc_rss() {
        // BBC RSSファイルからリンクを抽出するテスト
        let result = extract_links_from_rss("mock/rss/bbc.rss");

        assert!(result.is_ok(), "RSSファイルの読み込みに失敗しました");

        let articles = result.unwrap();
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
        let result = extract_links_from_rss("mock/rss/cbs.rss");

        assert!(result.is_ok(), "CBS RSSファイルの読み込みに失敗しました");

        let articles = result.unwrap();
        assert!(!articles.is_empty(), "抽出された記事が0件でした");

        // 記事の構造をチェック
        for article in &articles[..3.min(articles.len())] {
            // 最初の3記事をチェック
            assert!(!article.title.is_empty(), "記事のタイトルが空です");
            assert!(!article.link.is_empty(), "記事のリンクが空です");
            assert!(
                article.link.starts_with("http"),
                "リンクがHTTP形式ではありません"
            );
        }

        println!(
            "CBSテスト結果: {}件の記事を正常に抽出しました",
            articles.len()
        );
    }

    #[test]
    fn test_extract_links_from_guardian_rss() {
        // Guardian RSSファイルからリンクを抽出するテスト
        let result = extract_links_from_rss("mock/rss/guardian.rss");

        assert!(
            result.is_ok(),
            "Guardian RSSファイルの読み込みに失敗しました"
        );

        let articles = result.unwrap();
        assert!(!articles.is_empty(), "抽出された記事が0件でした");

        // 記事の構造をチェック
        for article in &articles[..3.min(articles.len())] {
            // 最初の3記事をチェック
            assert!(!article.title.is_empty(), "記事のタイトルが空です");
            assert!(!article.link.is_empty(), "記事のリンクが空です");
            assert!(
                article.link.starts_with("http"),
                "リンクがHTTP形式ではありません"
            );
        }

        println!(
            "Guardianテスト結果: {}件の記事を正常に抽出しました",
            articles.len()
        );
    }

    #[test]
    fn test_invalid_file_path() {
        // 存在しないファイルのテスト
        let result = extract_links_from_rss("non_existent_file.rss");
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
