use crate::domain::feed::{load_feeds_from_yaml, search_feeds, Feed, FeedQuery};
use crate::domain::rss::{extract_rss_links_from_channel, store_rss_links, RssLink};
use crate::domain::article::{store_article, Article};
use crate::infra::parser::parse_channel_from_xml_str;
use anyhow::{Context, Result};
use reqwest::Client;
use scraper::{Html, Selector};
use sqlx::PgPool;

/// RSSワークフローのメイン実行関数
/// 
/// 1. feeds.yamlからフィード設定を読み込み
/// 2. 各RSSフィードからリンクを取得してDBに保存
/// 3. 未処理のリンクから記事内容を取得してDBに保存
pub async fn execute_rss_workflow(pool: &PgPool) -> Result<()> {
    println!("=== RSSワークフロー開始 ===");

    // feeds.yamlからフィード設定を読み込み
    let feeds = load_feeds_from_yaml("src/domain/data/feeds.yaml")
        .context("フィード設定の読み込みに失敗")?;
    
    println!("フィード設定読み込み完了: {}件", feeds.len());

    // HTTPクライアントを作成
    let client = Client::new();

    // 段階1: RSSフィードからリンクを取得
    fetch_rss_links(&client, &feeds, pool).await?;

    // 段階2: 未処理のリンクから記事内容を取得
    fetch_article_contents(&client, pool).await?;

    println!("=== RSSワークフロー完了 ===");
    Ok(())
}

/// 特定のグループのRSSワークフローを実行
pub async fn execute_rss_workflow_for_group(pool: &PgPool, group: &str) -> Result<()> {
    println!("=== RSSワークフロー開始（グループ: {}）===", group);

    // feeds.yamlからフィード設定を読み込み
    let feeds = load_feeds_from_yaml("src/domain/data/feeds.yaml")
        .context("フィード設定の読み込みに失敗")?;
    
    // 指定されたグループのフィードのみを抽出
    let query = FeedQuery {
        group: Some(group.to_string()),
        name: None,
    };
    let filtered_feeds = search_feeds(&feeds, Some(query));
    
    if filtered_feeds.is_empty() {
        println!("指定されたグループ '{}' のフィードが見つかりませんでした", group);
        return Ok(());
    }

    println!("対象フィード数: {}件", filtered_feeds.len());

    // HTTPクライアントを作成
    let client = Client::new();

    // 段階1: RSSフィードからリンクを取得
    fetch_rss_links(&client, &filtered_feeds, pool).await?;

    // 段階2: 未処理のリンクから記事内容を取得
    fetch_article_contents(&client, pool).await?;

    println!("=== RSSワークフロー完了（グループ: {}）===", group);
    Ok(())
}

/// RSSフィードからリンクを取得してDBに保存
async fn fetch_rss_links(client: &Client, feeds: &[Feed], pool: &PgPool) -> Result<()> {
    println!("--- RSSフィードからリンク取得開始 ---");
    
    for feed in feeds {
        println!("フィード処理中: {} - {}", feed.group, feed.name);
        
        match fetch_single_rss_feed(client, feed).await {
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

/// 単一のRSSフィードを取得してRssLinkのベクタを返す
async fn fetch_single_rss_feed(client: &Client, feed: &Feed) -> Result<Vec<RssLink>> {
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

    let channel = parse_channel_from_xml_str(&xml_content)
        .context("XMLの解析に失敗")?;

    let rss_links = extract_rss_links_from_channel(&channel);
    
    Ok(rss_links)
}

/// 未処理のリンクから記事内容を取得してDBに保存
async fn fetch_article_contents(client: &Client, pool: &PgPool) -> Result<()> {
    println!("--- 記事内容取得開始 ---");
    
    // 未処理のリンクを取得（articleテーブルに存在しないrss_linkを取得）
    let unprocessed_links = get_unprocessed_rss_links(pool).await?;
    
    println!("未処理リンク数: {}件", unprocessed_links.len());
    
    for rss_link in unprocessed_links {
        println!("記事処理中: {}", rss_link.link);
        
        match fetch_single_article(client, &rss_link.link).await {
            Ok(article) => {
                match store_article(&article, pool).await {
                    Ok(result) => {
                        println!("  記事保存結果: {}", result);
                    }
                    Err(e) => {
                        eprintln!("  記事保存エラー: {}", e);
                    }
                }
            }
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

/// 単一の記事を取得してArticleを返す
async fn fetch_single_article(client: &Client, url: &str) -> Result<Article> {
    let response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .context(format!("記事の取得に失敗: {}", url))?;

    let status_code = response.status().as_u16() as i32;
    
    if !response.status().is_success() {
        return Ok(Article {
            url: url.to_string(),
            timestamp: chrono::Utc::now(),
            status_code,
            content: format!("HTTPエラー: {}", response.status()),
        });
    }

    let html_content = response
        .text()
        .await
        .context("レスポンステキストの取得に失敗")?;

    // HTMLから記事本文を抽出
    let content = extract_article_content(&html_content)?;

    Ok(Article {
        url: url.to_string(),
        timestamp: chrono::Utc::now(),
        status_code,
        content,
    })
}

/// HTMLから記事本文を抽出
fn extract_article_content(html: &str) -> Result<String> {
    let document = Html::parse_document(html);
    
    // よくある記事本文セレクタを試行
    let selectors = [
        "article",
        ".article-content",
        ".content",
        ".post-content",
        ".entry-content",
        "main",
        "#content",
        ".story-body",
    ];
    
    for selector_str in &selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(element) = document.select(&selector).next() {
                let text = element.text().collect::<Vec<_>>().join(" ");
                if text.len() > 100 { // 十分な長さのコンテンツのみ採用
                    return Ok(text);
                }
            }
        }
    }
    
    // フォールバック: bodyタグの内容を取得
    if let Ok(selector) = Selector::parse("body") {
        if let Some(element) = document.select(&selector).next() {
            let text = element.text().collect::<Vec<_>>().join(" ");
            return Ok(text);
        }
    }
    
    // 最終フォールバック: HTMLをそのまま返す
    Ok(html.to_string())
}

/// 未処理のRSSリンクを取得
async fn get_unprocessed_rss_links(pool: &PgPool) -> Result<Vec<RssLink>> {
    let links = sqlx::query_as!(
        RssLink,
        r#"
        SELECT rl.link, rl.title, rl.pub_date
        FROM rss_links rl
        LEFT JOIN articles a ON rl.link = a.url
        WHERE a.url IS NULL OR a.status_code != 200
        ORDER BY rl.pub_date DESC
        LIMIT 100
        "#
    )
    .fetch_all(pool)
    .await
    .context("未処理RSSリンクの取得に失敗")?;

    Ok(links)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path_regex};

    // テスト用のモックサーバーを構築
    async fn setup_mock_server() -> (MockServer, Vec<Feed>) {
        let mock_server = MockServer::start().await;
        
        // BBCのRSSフィードをモック
        let bbc_rss = std::fs::read_to_string("mock/rss/bbc.rss")
            .expect("BBCのモックRSSファイルが見つかりません");
        
        Mock::given(method("GET"))
            .and(path_regex(r"/bbc.*"))
            .respond_with(ResponseTemplate::new(200).set_body_string(bbc_rss))
            .mount(&mock_server)
            .await;

        // テスト用のHTMLレスポンス
        let test_html = r#"
            <html>
                <body>
                    <article>
                        <h1>テスト記事タイトル</h1>
                        <p>これはテスト記事の内容です。記事の本文がここに表示されます。</p>
                        <p>複数の段落で構成された記事の内容をテストします。</p>
                    </article>
                </body>
            </html>
        "#;
        
        Mock::given(method("GET"))
            .and(path_regex(r"/article.*"))
            .respond_with(ResponseTemplate::new(200).set_body_string(test_html))
            .mount(&mock_server)
            .await;

        let test_feeds = vec![
            Feed {
                group: "bbc".to_string(),
                name: "top".to_string(),
                link: format!("{}/bbc/rss.xml", mock_server.uri()),
            }
        ];

        (mock_server, test_feeds)
    }

    #[tokio::test]
    async fn test_fetch_single_rss_feed() {
        let (_mock_server, feeds) = setup_mock_server().await;
        let client = Client::new();
        
        let result = fetch_single_rss_feed(&client, &feeds[0]).await;
        assert!(result.is_ok(), "RSSフィードの取得に失敗");
        
        let rss_links = result.unwrap();
        assert!(!rss_links.is_empty(), "RSSリンクが取得されませんでした");
        
        println!("取得されたRSSリンク数: {}", rss_links.len());
    }

    #[tokio::test]
    async fn test_extract_article_content() {
        let html = r#"
            <html>
                <body>
                    <article>
                        <h1>テストタイトル</h1>
                        <p>テスト記事の内容です。</p>
                        <p>複数の段落があります。</p>
                    </article>
                </body>
            </html>
        "#;
        
        let result = extract_article_content(html);
        assert!(result.is_ok(), "記事内容の抽出に失敗");
        
        let content = result.unwrap();
        assert!(content.contains("テストタイトル"), "タイトルが含まれていません");
        assert!(content.contains("テスト記事の内容"), "記事内容が含まれていません");
        
        println!("抽出された内容: {}", content);
    }

    #[sqlx::test]
    async fn test_rss_workflow_integration(pool: sqlx::PgPool) -> Result<(), anyhow::Error> {
        
        // モックサーバーをセットアップ
        let mock_server = MockServer::start().await;
        
        // BBC RSSフィードをモック
        let bbc_rss = std::fs::read_to_string("mock/rss/bbc.rss")?;
        Mock::given(method("GET"))
            .and(path_regex(r"/bbc.*"))
            .respond_with(ResponseTemplate::new(200).set_body_string(bbc_rss))
            .mount(&mock_server)
            .await;

        // 記事HTMLをモック
        let test_html = r#"
            <html>
                <body>
                    <article>
                        <h1>統合テスト記事</h1>
                        <p>これは統合テスト用の記事です。十分な長さのコンテンツを提供します。</p>
                        <p>複数の段落で構成され、意味のあるコンテンツとして認識されるようにします。</p>
                        <p>RSSワークフローの統合テストが正しく動作することを確認するためのテスト記事です。</p>
                    </article>
                </body>
            </html>
        "#;
        
        Mock::given(method("GET"))
            .and(path_regex(r".*"))
            .respond_with(ResponseTemplate::new(200).set_body_string(test_html))
            .mount(&mock_server)
            .await;

        // テスト用フィード設定
        let test_feeds = vec![
            Feed {
                group: "bbc".to_string(),
                name: "top".to_string(),
                link: format!("{}/bbc/rss.xml", mock_server.uri()),
            }
        ];

        // HTTPクライアント作成
        let client = Client::new();

        // 段階1: RSSフィードからリンクを取得
        fetch_rss_links(&client, &test_feeds, &pool).await?;

        // データベースにRSSリンクが保存されたか確認
        let rss_count = sqlx::query_scalar!("SELECT COUNT(*) FROM rss_links")
            .fetch_one(&pool)
            .await?;
        
        assert!(rss_count.unwrap_or(0) > 0, "RSSリンクがデータベースに保存されませんでした");

        // 段階2: 記事内容を取得
        fetch_article_contents(&client, &pool).await?;

        // データベースに記事が保存されたか確認
        let article_count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
            .fetch_one(&pool)
            .await?;
        
        assert!(article_count.unwrap_or(0) > 0, "記事がデータベースに保存されませんでした");

        // 保存された記事の内容を確認
        let sample_article = sqlx::query_as!(
            Article,
            "SELECT url, timestamp, status_code, content FROM articles LIMIT 1"
        )
        .fetch_one(&pool)
        .await?;

        assert_eq!(sample_article.status_code, 200, "記事のステータスコードが200ではありません");
        assert!(!sample_article.content.is_empty(), "記事内容が空です");
        assert!(sample_article.content.len() > 50, "記事内容が短すぎます");

        println!("✅ RSS統合テスト成功");
        println!("  - RSSリンク数: {}", rss_count.unwrap_or(0));
        println!("  - 記事数: {}", article_count.unwrap_or(0));
        println!("  - サンプル記事URL: {}", sample_article.url);

        Ok(())
    }
}