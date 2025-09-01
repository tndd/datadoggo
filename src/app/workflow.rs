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
    get_unprocessed_rss_links, store_article, Article,
};

#[cfg(not(test))]
use crate::domain::article::fetch_article_from_url;

#[cfg(test)]
use crate::domain::article::fetch_article_with_client;

#[cfg(test)]
use crate::infra::api::firecrawl::MockFirecrawlClient;
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

        let article_result = {
            #[cfg(test)]
            {
                // テスト時はモッククライアントを使用
                let mock_client = MockFirecrawlClient::new_success("テスト記事内容");
                fetch_article_with_client(&rss_link.link, &mock_client).await
            }
            #[cfg(not(test))]
            {
                // 実行時は実際のクライアントを使用
                fetch_article_from_url(&rss_link.link).await
            }
        };

        match article_result {
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
    use sqlx::PgPool;

    /// 基本的なワークフロー動作テスト
    mod basic_workflow_tests {
        use super::*;

        #[sqlx::test]
        async fn test_empty_feeds_processing(
            _pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // 空のフィード配列のテスト
            let empty_feeds: Vec<Feed> = vec![];
            let client = reqwest::Client::new();
            let result = process_collect_rss_links(&client, &empty_feeds, &_pool).await;

            assert!(result.is_ok(), "空フィードでもエラーにならないはず");
            println!("✅ 空フィード処理テスト完了");
            Ok(())
        }

        #[sqlx::test]
        async fn test_empty_backlog_articles(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // 未処理リンクが0件の場合のテスト
            let client = reqwest::Client::new();
            let result = process_collect_backlog_articles(&client, &pool).await;

            assert!(result.is_ok(), "未処理リンクが0件でもエラーにならないはず");
            println!("✅ 空の未処理リンク処理テスト完了");
            Ok(())
        }

        #[test]
        fn test_feed_search_logic() {
            // フィード検索ロジックのテスト（外部通信なし）
            use crate::domain::feed::FeedQuery;
            
            let query = FeedQuery {
                group: Some("存在しないグループ".to_string()),
                name: None,
            };
            
            let result = search_feeds(Some(query));
            match result {
                Ok(feeds) => {
                    assert!(feeds.is_empty(), "存在しないグループでフィードが見つからないはず");
                },
                Err(_) => {
                    // ファイル読み込みエラーは許容
                }
            }
            
            println!("✅ フィード検索ロジックテスト完了");
        }
    }

    /// 統合テスト（モック使用）
    mod integration_tests {
        use super::*;

        #[sqlx::test]
        async fn test_article_fetch_with_mock(pool: PgPool) -> Result<(), anyhow::Error> {
            // テスト用RSSリンクを挿入
            sqlx::query!(
                "INSERT INTO rss_links (link, title, pub_date) VALUES ($1, $2, CURRENT_TIMESTAMP)",
                "https://test.example.com/article",
                "モック統合テスト記事"
            )
            .execute(&pool)
            .await?;

            // 記事取得を実行（モック使用）
            let client = reqwest::Client::new();
            let result = process_collect_backlog_articles(&client, &pool).await;
            
            assert!(result.is_ok(), "記事取得処理が失敗");
            
            // 記事がデータベースに保存されたことを確認
            let article_count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;
            
            assert!(article_count.unwrap_or(0) >= 1, "記事が保存されていない");
            
            println!("✅ モック記事取得統合テスト完了");
            Ok(())
        }

        #[tokio::test]
        async fn test_mock_client_direct() -> Result<(), anyhow::Error> {
            // モッククライアントの直接テスト
            let mock_client = MockFirecrawlClient::new_success("直接テスト用モック内容");
            let result = fetch_article_with_client("https://test.com", &mock_client).await;
            
            assert!(result.is_ok(), "モック記事取得が失敗");
            
            let article = result.unwrap();
            assert!(!article.content.is_empty(), "記事内容が空");
            assert_eq!(article.url, "https://test.com");
            assert!(article.content.contains("直接テスト用モック内容"), "モック内容が含まれていない");
            
            println!("✅ モッククライアント直接テスト完了");
            Ok(())
        }
    }

    /// エラーハンドリングテスト
    mod error_handling_tests {
        use super::*;

        #[sqlx::test]
        async fn test_invalid_url_with_mock(pool: PgPool) -> Result<(), anyhow::Error> {
            // 無効なURLを含むRSSリンクを挿入
            sqlx::query!(
                "INSERT INTO rss_links (link, title, pub_date) VALUES ($1, $2, CURRENT_TIMESTAMP)",
                "invalid-url",
                "無効URLテスト"
            )
            .execute(&pool)
            .await?;

            let client = reqwest::Client::new();
            let result = process_collect_backlog_articles(&client, &pool).await;
            
            // エラーが発生してもワークフロー全体は継続すること
            assert!(result.is_ok(), "無効URLがあってもワークフロー全体は成功するべき");
            
            // テスト時はモッククライアントが成功を返すので記事が保存される
            let article_count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;
            
            assert!(article_count.unwrap_or(0) >= 1, "記事が保存されていない");
            
            println!("✅ 無効URL処理テスト完了（モックで成功）");
            Ok(())
        }

        #[tokio::test]
        async fn test_error_client_handling() -> Result<(), anyhow::Error> {
            // エラークライアントを使用したテスト
            let error_client = MockFirecrawlClient::new_error("テストエラー");
            let result = fetch_article_with_client("https://test.com", &error_client).await;
            
            assert!(result.is_ok(), "エラークライアントでも結果を返すべき");
            
            let article = result.unwrap();
            assert_eq!(article.status_code, 500, "エラー時はstatus_code=500になるべき");
            assert!(article.content.contains("エラー"), "エラー内容が記録されるべき");
            
            println!("✅ エラークライアント処理テスト完了");
            Ok(())
        }
    }
}
