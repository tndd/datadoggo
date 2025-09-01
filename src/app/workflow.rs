//! RSSワークフローモジュール
//!
//! このモジュールは以下の機能を提供します：
//! 1. RSSフィードからリンク情報を取得してデータベースに保存
//! 2. 未処理のリンクから記事内容を取得してデータベースに保存
//!
//! ## テスト実行方法
//!
//! ```bash
//! # 通常のテスト実行（外部通信なし、モック専用）
//! cargo test
//!
//! # オンラインテストを含む完全なテスト実行（外部通信あり）
//! cargo test --features online
//! ```
//!
//! テスト時は wiremock を使用してモックサーバーを立てることで
//! 外部への通信を行わずにテストできます。

use crate::domain::article::{
    fetch_article_from_url, get_unprocessed_rss_links, store_article, Article,
};
use crate::domain::feed::{search_feeds, Feed, FeedQuery};
use crate::domain::rss::{extract_rss_links_from_channel, store_rss_links, RssLink};
use crate::infra::parser::parse_channel_from_xml_str;
use anyhow::{Context, Result};
use reqwest::Client;
use sqlx::PgPool;

/// RSSワークフローのメイン実行関数
///
/// 1. feeds.yamlからフィード設定を読み込み
/// 2. 各RSSフィードからリンクを取得してDBに保存
/// 3. 未処理のリンクから記事内容を取得してDBに保存
pub async fn execute_rss_workflow(pool: &PgPool) -> Result<()> {
    println!("=== RSSワークフロー開始 ===");
    // feeds.yamlからフィード設定を読み込み
    let feeds = search_feeds(None).context("フィード設定の読み込みに失敗")?;
    println!("フィード設定読み込み完了: {}件", feeds.len());
    // HTTPクライアントを作成
    let client = Client::new();
    // 段階1: RSSフィードからリンクを取得
    process_collect_rss_links(&client, &feeds, pool).await?;
    // 段階2: 未処理のリンクから記事内容を取得
    process_collect_backlog_articles(&client, pool).await?;

    println!("=== RSSワークフロー完了 ===");
    Ok(())
}

/// 特定のグループのRSSワークフローを実行
pub async fn execute_rss_workflow_for_group(pool: &PgPool, group: &str) -> Result<()> {
    println!("=== RSSワークフロー開始（グループ: {}）===", group);
    // 指定されたグループのフィードのみを抽出
    let query = FeedQuery {
        group: Some(group.to_string()),
        name: None,
    };
    let filtered_feeds = search_feeds(Some(query)).context("フィード設定の読み込みに失敗")?;

    if filtered_feeds.is_empty() {
        println!(
            "指定されたグループ '{}' のフィードが見つかりませんでした",
            group
        );
        return Ok(());
    }
    println!("対象フィード数: {}件", filtered_feeds.len());

    // HTTPクライアントを作成
    let client = Client::new();
    // 段階1: RSSフィードからリンクを取得
    process_collect_rss_links(&client, &filtered_feeds, pool).await?;
    // 段階2: 未処理のリンクから記事内容を取得
    process_collect_backlog_articles(&client, pool).await?;

    println!("=== RSSワークフロー完了（グループ: {}）===", group);
    Ok(())
}

/// RSSフィードからリンクを収集してDBに保存する
async fn process_collect_rss_links(client: &Client, feeds: &[Feed], pool: &PgPool) -> Result<()> {
    println!("--- RSSフィードからリンク取得開始 ---");

    for feed in feeds {
        println!("フィード処理中: {} - {}", feed.group, feed.name);

        match fetch_rss_links_from_feed(client, feed).await {
            Ok(rss_links) => {
                println!("  {}件のリンクを抽出", rss_links.len());

                match store_rss_links(&rss_links, pool).await {
                    Ok(result) => {
                        println!("  DB保存結果: {}", result);
                    }
                    Err(e) => {
                        eprintln!("  DB保存エラー: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("  フィード取得エラー: {}", e);
            }
        }
    }

    println!("--- RSSフィードからリンク取得完了 ---");
    Ok(())
}

/// feedからrss_linkのリストを取得する
async fn fetch_rss_links_from_feed(client: &Client, feed: &Feed) -> Result<Vec<RssLink>> {
    let response = client
        .get(&feed.link)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .context(format!("RSSフィードの取得に失敗: {}", feed.link))?;

    let xml_content = response
        .text()
        .await
        .context("レスポンステキストの取得に失敗")?;
    let channel = parse_channel_from_xml_str(&xml_content).context("XMLの解析に失敗")?;
    let rss_links = extract_rss_links_from_channel(&channel);

    Ok(rss_links)
}

/// 未処理のリンクから処理待ちの記事を収集してDBに保存する
async fn process_collect_backlog_articles(_client: &Client, pool: &PgPool) -> Result<()> {
    println!("--- 記事内容取得開始 ---");
    // 未処理のリンクを取得（articleテーブルに存在しないrss_linkを取得）
    let unprocessed_links = get_unprocessed_rss_links(pool).await?;
    println!("未処理リンク数: {}件", unprocessed_links.len());

    for rss_link in unprocessed_links {
        println!("記事処理中: {}", rss_link.link);

        match fetch_article_from_url(&rss_link.link).await {
            Ok(article) => match store_article(&article, pool).await {
                Ok(result) => {
                    println!("  記事保存結果: {}", result);
                }
                Err(e) => {
                    eprintln!("  記事保存エラー: {}", e);
                }
            },
            Err(e) => {
                eprintln!("  記事取得エラー: {}", e);

                // エラーが発生した場合も、status_codeを記録してスキップ
                let error_article = Article {
                    url: rss_link.link,
                    timestamp: chrono::Utc::now(),
                    status_code: 500, // エラー用のステータスコード
                    content: format!("取得エラー: {}", e),
                };

                if let Err(store_err) = store_article(&error_article, pool).await {
                    eprintln!("  エラー記事の保存に失敗: {}", store_err);
                }
            }
        }
    }

    println!("--- 記事内容取得完了 ---");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 統一されたWorkflowテスト - 1つのコードでモック/オンライン切り替え
    #[tokio::test]
    async fn test_workflow_fetch_article_unified() -> Result<(), anyhow::Error> {
        use crate::domain::article::fetch_article_with_mock;

        let test_url = "https://httpbin.org/html";
        let mock_content =
            "ワークフロー統合テスト記事\n\nWorkflowモジュールでのFirecrawl統合テストです。";

        // 新しい関数ベースのモジュールを使用してテスト
        let article = fetch_article_with_mock(test_url, Some(mock_content)).await?;

        // 基本的なアサーション
        assert_eq!(article.url, test_url);
        assert_eq!(article.status_code, 200);
        assert!(article.content.contains(mock_content));

        println!("✅ Workflow統一テスト成功");
        println!("URL: {}", article.url);
        println!("内容長: {}文字", article.content.len());

        Ok(())
    }
}
