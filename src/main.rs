mod db_writer;
mod rss_reader;

use db_writer::*;
use rss_reader::*;

#[tokio::main]
async fn main() {
    // 環境変数を読み込み（.envファイルがあれば使用）
    let _ = dotenvy::dotenv();
    match read_channel_from_file("mock/rss/bbc.rss") {
        Ok(channel) => {
            let articles = extract_rss_articles_from_channel(&channel);
            println!("BBCのRSSから{}件の記事を抽出しました。", articles.len());

            match save_articles_to_db(&articles).await {
                Ok(_) => println!("{}件の記事をPostgreSQLに保存しました。", articles.len()),
                Err(e) => eprintln!("データベースへの保存中にエラーが発生しました: {}", e),
            }
        }
        Err(e) => {
            eprintln!("RSSの読み込み中にエラーが発生しました: {}", e);
        }
    }
}
