mod domain;
mod infra;

use domain::rss::{extract_rss_links_from_channel, store_rss_links};

use infra::db::setup_database;
use infra::loader::{load_channel_from_xml_file, load_json_from_file};

#[tokio::main]
async fn main() {
    // 環境変数を読み込み（.envファイルがあれば使用）
    let _ = dotenvy::dotenv();

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
