use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

// Firecrawl記事の情報を格納する構造体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirecrawlArticle {
    pub markdown: String,
    pub metadata: FirecrawlMetadata,
}

// Firecrawlのメタデータを格納する構造体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirecrawlMetadata {
    pub favicon: Option<String>,
    #[serde(rename = "page.section")]
    pub page_section: Option<String>,
    pub viewport: Option<Vec<String>>,
    #[serde(rename = "og:image:alt")]
    pub og_image_alt: Option<String>,
    #[serde(rename = "theme-color")]
    pub theme_color: Option<Vec<String>>,
    pub title: Option<String>,
    #[serde(rename = "al:android:package")]
    pub al_android_package: Option<String>,
    #[serde(rename = "page.subsection")]
    pub page_subsection: Option<String>,
    #[serde(rename = "ogTitle")]
    pub og_title: Option<String>,
    #[serde(rename = "next-head-count")]
    pub next_head_count: Option<String>,
    #[serde(rename = "al:ios:app_store_id")]
    pub al_ios_app_store_id: Option<String>,
    #[serde(rename = "og:image")]
    pub og_image: Option<String>,
    #[serde(rename = "og:description")]
    pub og_description: Option<String>,
    #[serde(rename = "ogDescription")]
    pub og_description_alt: Option<String>,
    pub robots: Option<String>,
    #[serde(rename = "ogImage")]
    pub og_image_alt_field: Option<String>,
    #[serde(rename = "twitter:image:src")]
    pub twitter_image_src: Option<String>,
    #[serde(rename = "al:android:app_name")]
    pub al_android_app_name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "al:ios:app_name")]
    pub al_ios_app_name: Option<String>,
    pub language: Option<String>,
    #[serde(rename = "msapplication-TileColor")]
    pub msapplication_tile_color: Option<String>,
    #[serde(rename = "al:web:url")]
    pub al_web_url: Option<String>,
    #[serde(rename = "article:modified_time")]
    pub article_modified_time: Option<String>,
    #[serde(rename = "cXenseParse:publishtime")]
    pub cxense_parse_publishtime: Option<String>,
    #[serde(rename = "cXenseParse:author")]
    pub cxense_parse_author: Option<String>,
    #[serde(rename = "twitter:card")]
    pub twitter_card: Option<String>,
    #[serde(rename = "google-site-verification")]
    pub google_site_verification: Option<String>,
    #[serde(rename = "color-scheme")]
    pub color_scheme: Option<String>,
    #[serde(rename = "twitter:description")]
    pub twitter_description: Option<String>,
    pub version: Option<String>,
    #[serde(rename = "og:title")]
    pub og_title_alt: Option<String>,
    #[serde(rename = "al:ios:url")]
    pub al_ios_url: Option<String>,
    #[serde(rename = "twitter:image:alt")]
    pub twitter_image_alt: Option<String>,
    #[serde(rename = "cXenseParse:pageclass")]
    pub cxense_parse_pageclass: Option<String>,
    #[serde(rename = "al:android:url")]
    pub al_android_url: Option<String>,
    #[serde(rename = "apple-itunes-app")]
    pub apple_itunes_app: Option<String>,
    #[serde(rename = "twitter:title")]
    pub twitter_title: Option<String>,
    #[serde(rename = "scrapeId")]
    pub scrape_id: Option<String>,
    #[serde(rename = "sourceURL")]
    pub source_url: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "statusCode")]
    pub status_code: Option<i32>,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
    #[serde(rename = "proxyUsed")]
    pub proxy_used: Option<String>,
    #[serde(rename = "cacheState")]
    pub cache_state: Option<String>,
    #[serde(rename = "cachedAt")]
    pub cached_at: Option<String>,
    #[serde(rename = "creditsUsed")]
    pub credits_used: Option<i32>,
    
    // その他のフィールドをキャッチするため
    #[serde(flatten)]
    pub additional_fields: HashMap<String, serde_json::Value>,
}

// ファイルからFirecrawlデータを読み込むヘルパー関数
pub fn read_firecrawl_from_file(
    file_path: &str,
) -> Result<FirecrawlArticle, Box<dyn std::error::Error>> {
    let file = File::open(file_path)?;
    let buf_reader = BufReader::new(file);
    let article: FirecrawlArticle = serde_json::from_reader(buf_reader)?;
    Ok(article)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_firecrawl_from_file() {
        // BBCのモックファイルを読み込んでパース
        let result = read_firecrawl_from_file("mock/fc/bbc.json");
        assert!(result.is_ok(), "Firecrawl JSONファイルの読み込みに失敗");

        let article = result.unwrap();
        
        // 基本的なフィールドの検証
        assert!(!article.markdown.is_empty(), "markdownが空です");
        assert!(article.metadata.title.is_some(), "タイトルがありません");
        assert!(article.metadata.url.is_some(), "URLがありません");
        
        println!("✅ Firecrawlデータの読み込みテスト成功");
        println!("タイトル: {:?}", article.metadata.title);
        println!("URL: {:?}", article.metadata.url);
        println!("Markdownサイズ: {} characters", article.markdown.len());
    }

    #[test]
    fn test_read_non_existing_file() {
        // 存在しないファイルを読み込もうとするテスト
        let result = read_firecrawl_from_file("non_existent_file.json");
        assert!(result.is_err(), "存在しないファイルでエラーにならなかった");
    }
}