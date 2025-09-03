/// NOTE: main.rsは単なる最小限の動作確認に過ぎないので凝った実装をしないように
use datadoggo::{app, domain, infra};

use app::workflow::execute_rss_workflow;
use domain::feed::{search_feeds, FeedQuery};
use domain::rss::{get_rss_links_from_channel, store_rss_links};
use infra::api::firecrawl::ReqwestFirecrawlClient;
use infra::api::http::ReqwestHttpClient;
use infra::storage::db::setup_database;
use infra::storage::file::{load_channel_from_xml_file, load_json_from_file};

#[tokio::main]
async fn main() {
    // 環境変数を読み込み（.envファイルがあれば使用）
    let _ = dotenvy::dotenv();

    // フィード設定を読み込み
    println!("=== フィード設定の読み込み ===");
    match search_feeds(None) {
        Ok(feeds) => {
            println!("全フィード数: {}", feeds.len());

            // BBCフィードの例
            let bbc_query = Some(FeedQuery::from_group("bbc"));
            match search_feeds(bbc_query) {
                Ok(bbc_feeds) => {
                    println!("BBCフィード数: {}", bbc_feeds.len());
                    for feed in bbc_feeds.iter().take(3) {
                        println!("  - {}: {}", feed.name, feed.link);
                    }
                }
                Err(e) => eprintln!("BBCフィード検索エラー: {}", e),
            }

            // Yahoo Japanフィードの例
            let yahoo_query = Some(FeedQuery::from_group("yahoo_japan"));
            match search_feeds(yahoo_query) {
                Ok(yahoo_feeds) => {
                    println!("Yahoo Japanフィード数: {}", yahoo_feeds.len());
                    for feed in yahoo_feeds.iter().take(3) {
                        println!("  - {}: {}", feed.name, feed.link);
                    }
                }
                Err(e) => eprintln!("Yahoo Japanフィード検索エラー: {}", e),
            }
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
            let links = get_rss_links_from_channel(&channel);
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

    // RSSワークフローのデモンストレーション（BBCグループのみ）
    println!("\n=== RSSワークフロー（デモ実行） ===");
    // 本番用のクライアントをインスタンス化
    let http_client = ReqwestHttpClient::new();
    let firecrawl_client =
        ReqwestFirecrawlClient::new().expect("Firecrawlクライアントの初期化に失敗");

    match execute_rss_workflow(&http_client, &firecrawl_client, &pool, Some("bbc")).await {
        Ok(()) => {
            println!("RSSワークフローが正常に完了しました");
        }
        Err(e) => {
            eprintln!("RSSワークフローでエラーが発生しました: {}", e);
        }
    }
}
