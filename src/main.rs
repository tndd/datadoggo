mod db_common;
mod rss_reader;
mod rss_db_writer;
mod firecrawl_reader;
mod firecrawl_db_writer;

use rss_reader::*;
use rss_db_writer::*;
use firecrawl_reader::*;
use firecrawl_db_writer::*;

#[tokio::main]
async fn main() {
    // 環境変数を読み込み（.envファイルがあれば使用）
    let _ = dotenvy::dotenv();
    
    // RSS処理
    println!("=== RSS処理を開始 ===");
    match read_channel_from_file("mock/rss/bbc.rss") {
        Ok(channel) => {
            let articles = extract_rss_articles_from_channel(&channel);
            println!("BBCのRSSから{}件の記事を抽出しました。", articles.len());

            match save_rss_articles_to_db(&articles).await {
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
    match read_firecrawl_from_file("mock/fc/bbc.json") {
        Ok(article) => {
            println!("BBCのFirecrawlデータを読み込みました。");
            println!("タイトル: {:?}", article.metadata.title);
            println!("URL: {:?}", article.metadata.url);
            println!("Markdownサイズ: {} characters", article.markdown.len());

            match save_firecrawl_article_to_db(&article).await {
                Ok(result) => {
                    println!("{}", result);
                }
                Err(e) => eprintln!("Firecrawlデータのデータベース保存中にエラーが発生しました: {}", e),
            }
        }
        Err(e) => {
            eprintln!("Firecrawlデータの読み込み中にエラーが発生しました: {}", e);
        }
    }
}
