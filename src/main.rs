mod domain;
mod infra;

use domain::feed::FeedConfig;
use domain::rss::{extract_rss_links_from_channel, store_rss_links};

use infra::db::setup_database;
use infra::loader::{load_channel_from_xml_file, load_json_from_file};

#[tokio::main]
async fn main() {
    // 環境変数を読み込み（.envファイルがあれば使用）
    let _ = dotenvy::dotenv();

    // フィード設定を読み込み
    println!("=== フィード設定の読み込み ===");
    match FeedConfig::load_default() {
        Ok(config) => {
            let groups = config.get_groups();
            println!("利用可能なフィードグループ: {:?}", groups);
            
            // BBCフィードの例
            let bbc_feeds = config.get_feeds_by_group("bbc");
            println!("BBCフィード数: {}", bbc_feeds.len());
            for feed in bbc_feeds.iter().take(3) {
                println!("  - {}: {}", feed.name, feed.link);
            }
            
            // Yahoo Japanフィードの例
            let yahoo_feeds = config.get_feeds_by_group("yahoo_japan");
            println!("Yahoo Japanフィード数: {}", yahoo_feeds.len());
            for feed in yahoo_feeds.iter().take(3) {
                println!("  - {}: {}", feed.name, feed.link);
            }
            
            println!("全フィード数: {}", config.get_all_feeds().len());
        }
        Err(e) => {
            eprintln!("フィード設定の読み込みに失敗しました: {}", e);
        }
    }

    // データベースプールを1回だけ作成
    let pool = match setup_database().await {
        Ok(pool) => pool,
        Err(e) => {
            eprintln!("データベースの初期化に失敗しました: {}", e);
            return;
        }
    };

    // RSS処理
    println!("=== RSS処理を開始 ===");
    match load_channel_from_xml_file("mock/rss/bbc.rss") {
        Ok(channel) => {
            let links = extract_rss_links_from_channel(&channel);
            println!("BBCのRSSから{}件のリンクを抽出しました。", links.len());

            match store_rss_links(&links, &pool).await {
                Ok(result) => {
                    println!("{}", result);
                }
                Err(e) => eprintln!("データベースへの保存中にエラーが発生しました: {}", e),
            }
        }
        Err(e) => {
            eprintln!("RSSの読み込み中にエラーが発生しました: {}", e);
        }
    }

    // Firecrawl処理（簡易確認）
    println!("\n=== Firecrawl処理を開始 ===");
    match load_json_from_file("mock/fc/bbc.json") {
        Ok(json_value) => {
            println!("BBCのFirecrawlデータを読み込みました。");
            if let Some(metadata) = json_value.get("metadata") {
                if let Some(url) = metadata.get("url").and_then(|v| v.as_str()) {
                    println!("URL: {}", url);
                }
            }
            println!("JSONデータの読み込み完了");
        }
        Err(e) => {
            eprintln!("Firecrawlデータの読み込み中にエラーが発生しました: {}", e);
        }
    }
}
