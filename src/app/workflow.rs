use crate::{
    domain::{
        article::{search_unprocessed_rss_links, store_article, Article},
        feed::{search_feeds, Feed, FeedQuery},
        rss::{extract_rss_links_from_channel, store_rss_links, RssLink},
    },
    infra::{api::http::HttpClientProtocol, parser::parse_channel_from_xml_str},
};
use anyhow::{Context, Result};
use sqlx::PgPool;

// --- Production/Online Test Imports ---
#[cfg(any(not(test), feature = "online"))]
use crate::{
    domain::article::fetch_article_from_url, infra::api::http::ReqwestHttpClient as HttpClient,
};

// --- Offline Test Imports ---
#[cfg(all(test, not(feature = "online")))]
use crate::{
    domain::article::fetch_article_with_client,
    infra::api::{firecrawl::MockFirecrawlClient, http::MockHttpClient as HttpClient},
};

/// RSSワークフローのメイン実行関数
///
/// 1. feeds.yamlからフィード設定を読み込み
/// 2. 各RSSフィードからリンクを取得してDBに保存
/// 3. 未処理のリンクから記事内容を取得してDBに保存
///
/// # 引数
/// * `pool` - データベース接続プール
/// * `group` - 処理対象のグループ（Noneの場合は全フィードを処理）
pub async fn execute_rss_workflow(pool: &PgPool, group: Option<&str>) -> Result<()> {
    match group {
        Some(group_name) => {
            println!("=== RSSワークフロー開始（グループ: {}）===", group_name);
        }
        None => {
            println!("=== RSSワークフロー開始 ===");
        }
    }

    // feeds.yamlからフィード設定を読み込み
    let query = group.map(|g| FeedQuery {
        group: Some(g.to_string()),
        name: None,
    });
    let feeds = search_feeds(query).context("フィード設定の読み込みに失敗")?;

    if let Some(group_name) = group {
        if feeds.is_empty() {
            println!(
                "指定されたグループ '{}' のフィードが見つかりませんでした",
                group_name
            );
            return Ok(());
        }
        println!("対象フィード数: {}件", feeds.len());
    } else {
        println!("フィード設定読み込み完了: {}件", feeds.len());
    }

    // HTTPクライアントを作成
    #[cfg(any(not(test), feature = "online"))]
    let http_client = HttpClient::new();

    #[cfg(all(test, not(feature = "online")))]
    let http_client = HttpClient::new_success("<rss><channel><item><title>テスト</title><link>https://test.com</link></item></channel></rss>");

    // 段階1: RSSフィードからリンクを取得
    process_collect_rss_links(&http_client, &feeds, pool).await?;
    // 段階2: 未処理のリンクから記事内容を取得
    process_collect_backlog_articles(pool).await?;

    match group {
        Some(group_name) => {
            println!("=== RSSワークフロー完了（グループ: {}）===", group_name);
        }
        None => {
            println!("=== RSSワークフロー完了 ===");
        }
    }
    Ok(())
}

/// RSSフィードからリンクを収集してDBに保存する
async fn process_collect_rss_links(
    client: &dyn HttpClientProtocol,
    feeds: &[Feed],
    pool: &PgPool,
) -> Result<()> {
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
async fn fetch_rss_links_from_feed(
    client: &dyn HttpClientProtocol,
    feed: &Feed,
) -> Result<Vec<RssLink>> {
    let xml_content = client
        .get_text(&feed.link, 30)
        .await
        .context(format!("RSSフィードの取得に失敗: {}", feed.link))?;
    let channel = parse_channel_from_xml_str(&xml_content).context("XMLの解析に失敗")?;
    let rss_links = extract_rss_links_from_channel(&channel);

    Ok(rss_links)
}

/// 未処理のリンクから処理待ちの記事を収集してDBに保存する
async fn process_collect_backlog_articles(pool: &PgPool) -> Result<()> {
    println!("--- 記事内容取得開始 ---");
    // 未処理のリンクを取得（articleテーブルに存在しないrss_linkを取得）
    let unprocessed_links = search_unprocessed_rss_links(pool).await?;
    println!("未処理リンク数: {}件", unprocessed_links.len());

    for rss_link in unprocessed_links {
        println!("記事処理中: {}", rss_link.link);

        let article_result = {
            #[cfg(all(test, not(feature = "online")))]
            {
                // 通常テスト時はモッククライアントを使用
                let mock_client = MockFirecrawlClient::new_success("テスト記事内容");
                fetch_article_with_client(&rss_link.link, &mock_client).await
            }
            #[cfg(any(not(test), feature = "online"))]
            {
                // 本番実行時またはオンラインテスト時は実際のクライアントを使用
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
            let result = process_collect_backlog_articles(&pool).await;

            assert!(result.is_ok(), "記事取得処理が失敗");

            // 記事がデータベースに保存されたことを確認
            let article_count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;

            assert!(article_count.unwrap_or(0) >= 1, "記事が保存されていない");

            println!("✅ モック記事取得統合テスト完了");
            Ok(())
        }
    }

    /// HTTPモックを使ったテスト
    mod http_mock_tests {
        use super::*;
        use crate::infra::api::http::MockHttpClient;

        #[tokio::test]
        async fn test_fetch_rss_links_with_mock() -> Result<(), anyhow::Error> {
            let rss_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
    <channel>
        <title>テストRSSフィード</title>
        <item>
            <title>記事1</title>
            <link>https://example.com/article1</link>
            <pubDate>Wed, 01 Jan 2025 12:00:00 GMT</pubDate>
        </item>
        <item>
            <title>記事2</title>
            <link>https://example.com/article2</link>
            <pubDate>Thu, 02 Jan 2025 12:00:00 GMT</pubDate>
        </item>
    </channel>
</rss>"#;

            // モッククライアントでRSSフィードを設定
            let mock_client = MockHttpClient::new_success(rss_xml);

            let test_feed = Feed {
                group: "test".to_string(),
                name: "テストフィード".to_string(),
                link: "https://example.com/rss.xml".to_string(),
            };

            let result = fetch_rss_links_from_feed(&mock_client, &test_feed).await;

            assert!(result.is_ok(), "RSSフィードの取得が失敗");

            let rss_links = result.unwrap();
            assert_eq!(rss_links.len(), 2, "2件のリンクが取得されるべき");

            let first_link = &rss_links[0];
            assert_eq!(first_link.link, "https://example.com/article1");
            assert_eq!(first_link.title, "記事1");

            println!("✅ HTTPモック使用のRSSフィード取得テスト完了");
            Ok(())
        }

        #[tokio::test]
        async fn test_fetch_rss_links_with_error_mock() -> Result<(), anyhow::Error> {
            // エラーを返すモッククライアント
            let error_client = MockHttpClient::new_error("接続タイムアウト");

            let test_feed = Feed {
                group: "test".to_string(),
                name: "エラーテストフィード".to_string(),
                link: "https://example.com/error.xml".to_string(),
            };

            let result = fetch_rss_links_from_feed(&error_client, &test_feed).await;

            assert!(result.is_err(), "エラーが発生するべき");
            let error_msg = result.unwrap_err().to_string();
            println!("エラーメッセージ: {}", error_msg);
            // エラーが正しく伝播されていることを確認
            assert!(error_msg.contains("RSSフィードの取得に失敗"));

            println!("✅ HTTPモック使用のエラーハンドリングテスト完了");
            Ok(())
        }

        #[tokio::test]
        async fn test_fetch_rss_links_with_invalid_xml() -> Result<(), anyhow::Error> {
            let invalid_xml = "<invalid>xml content</broken>";

            let mock_client = MockHttpClient::new_success(invalid_xml);

            let test_feed = Feed {
                group: "test".to_string(),
                name: "無効XMLテストフィード".to_string(),
                link: "https://example.com/invalid.xml".to_string(),
            };

            let result = fetch_rss_links_from_feed(&mock_client, &test_feed).await;

            // XMLパースエラーが発生するべき
            assert!(result.is_err(), "無効なXMLでエラーが発生するべき");

            println!("✅ 無効XMLハンドリングテスト完了");
            Ok(())
        }
    }
}
