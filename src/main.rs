mod article;
mod infra;
mod rss;

use article::*;
use rss::*;

#[tokio::main]
async fn main() {
    // 環境変数を読み込み（.envファイルがあれば使用）
    let _ = dotenvy::dotenv();

    // RSS処理
    println!("=== RSS処理を開始 ===");
    match read_channel_from_file("mock/rss/bbc.rss") {
        Ok(channel) => {
            let links = extract_rss_links_from_channel(&channel);
            println!("BBCのRSSから{}件のリンクを抽出しました。", links.len());

            match save_rss_links_to_db(&links).await {
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

    // Firecrawl処理
    println!("\n=== Firecrawl処理を開始 ===");
    match read_article_from_file("mock/fc/bbc.json") {
        Ok(article) => {
            println!("BBCのFirecrawlデータを読み込みました。");
            println!("URL: {}", article.url);
            println!("Status Code: {:?}", article.status_code);
            println!("Contentサイズ: {} characters", article.content.len());

            match save_article_to_db(&article).await {
                Ok(result) => {
                    println!("{}", result);
                }
                Err(e) => eprintln!(
                    "Firecrawlデータのデータベース保存中にエラーが発生しました: {}",
                    e
                ),
            }
        }
        Err(e) => {
            eprintln!("Firecrawlデータの読み込み中にエラーが発生しました: {}", e);
        }
    }
}
