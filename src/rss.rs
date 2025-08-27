use crate::services::db::{create_pool, initialize_database, SaveResult};
use crate::services::loader::load_file;
use rss::Channel;
use sqlx::{Error as SqlxError, PgPool};

// RSS記事の情報を格納する構造体
#[derive(Debug, Clone)]
pub struct RssArticle {
    pub title: String,
    pub link: String,
    pub description: Option<String>,
    pub pub_date: Option<String>,
}

// RSSのチャンネルから記事を抽出する関数
pub fn extract_rss_articles_from_channel(channel: &Channel) -> Vec<RssArticle> {
    let mut articles = Vec::new();

    for item in channel.items() {
        if let Some(link) = item.link() {
            let article = RssArticle {
                title: item.title().unwrap_or("タイトルなし").to_string(),
                link: link.to_string(),
                description: item.description().map(|d| d.to_string()),
                pub_date: item.pub_date().map(|d| d.to_string()),
            };
            articles.push(article);
        }
    }

    articles
}

// ファイルからRSSを読み込むヘルパー関数（loaderを使用）
pub fn read_channel_from_file(file_path: &str) -> Result<Channel, Box<dyn std::error::Error>> {
    let buf_reader = load_file(file_path)?;
    Channel::read_from(buf_reader).map_err(Into::into)
}

/// # 概要
/// RssArticleの配列をデータベースに保存する。
///
/// ## 動作
/// - 自動でデータベース接続プールを作成
/// - マイグレーションを実行
/// - RSS記事を一括保存
/// - 重複記事は保存をスキップ
///
/// ## 引数
/// - `articles`: 保存するRSS記事のスライス
///
/// ## 戻り値
/// 成功時は`SaveResult`構造体を返し、保存結果の詳細情報を提供する。
/// - `inserted`: 新規に挿入された記事数
/// - `skipped`: 重複によりスキップされた記事数
///
/// ## エラー
/// 操作失敗時にはSqlxErrorを返し、全ての操作をロールバックする。
pub async fn save_rss_articles_to_db(articles: &[RssArticle]) -> Result<SaveResult, SqlxError> {
    let pool = create_pool().await?;
    initialize_database(&pool).await?;
    save_rss_articles_with_pool(articles, &pool).await
}

/// # 概要
/// RssArticleの配列を指定されたデータベースプールに保存する。
/// 既にプールを準備している場合は `save_rss_articles_to_db` ではなく、この関数を使用する。
/// 
/// # Note
/// sqlxの推奨パターンに従い、sqlx::query!マクロを使用してコンパイル時安全性を確保しています。
pub async fn save_rss_articles_with_pool(
    articles: &[RssArticle],
    pool: &PgPool,
) -> Result<SaveResult, SqlxError> {
    if articles.is_empty() {
        return Ok(SaveResult {
            inserted: 0,
            skipped: 0,
        });
    }

    let mut tx = pool.begin().await?;
    let mut total_inserted = 0;

    // sqlx::query!マクロを使用してコンパイル時にSQLを検証
    for article in articles {
        let result = sqlx::query!(
            r#"
            INSERT INTO rss_articles (title, link, description, pub_date)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (link) DO NOTHING
            "#,
            article.title,
            article.link,
            article.description,
            article.pub_date
        )
        .execute(&mut *tx)
        .await?;
        
        if result.rows_affected() > 0 {
            total_inserted += 1;
        }
    }

    tx.commit().await?;

    Ok(SaveResult {
        inserted: total_inserted,
        skipped: articles.len() - total_inserted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    // XMLからRSSチャンネルを解析するヘルパー関数
    fn parse_channel_from_xml(xml: &str) -> Result<Channel, Box<dyn std::error::Error>> {
        Channel::read_from(BufReader::new(Cursor::new(xml.as_bytes()))).map_err(Into::into)
    }

    // 記事の基本構造をチェックするヘルパー関数
    fn validate_articles(articles: &[RssArticle]) {
        for article in &articles[..3.min(articles.len())] {
            assert!(!article.title.is_empty(), "記事のタイトルが空です");
            assert!(!article.link.is_empty(), "記事のリンクが空です");
            assert!(
                article.link.starts_with("http"),
                "リンクがHTTP形式ではありません"
            );
        }
    }

    #[test]
    fn test_extract_rss_articles_from_xml() {
        // xml->channel->rss_articleの流れの確認
        let xml: &str = r#"
            <rss version="2.0">
                <channel>
                    <title>Test Feed</title>
                    <link>http://example.com</link>
                    <description>Test Description</description>
                    <item>
                        <title>Test Article 1</title>
                        <link>http://example.com/article1</link>
                        <description>Test article 1 description</description>
                        <pubDate>Mon, 10 Aug 2025 12:00:00 +0000</pubDate>
                    </item>
                    <item>
                        <title>Test Article 2</title>
                        <link>http://example.com/article2</link>
                        <description>Test article 2 description</description>
                        <pubDate>Mon, 10 Aug 2025 13:00:00 +0000</pubDate>
                    </item>
                </channel>
            </rss>
            "#;
        let channel = parse_channel_from_xml(xml).expect("Failed to parse test RSS");
        let articles = extract_rss_articles_from_channel(&channel);

        assert_eq!(articles.len(), 2, "2件の記事が抽出されるはず");
        assert_eq!(articles[0].title, "Test Article 1");
        assert_eq!(articles[0].link, "http://example.com/article1");
        assert_eq!(articles[1].title, "Test Article 2");
        assert_eq!(articles[1].link, "http://example.com/article2");
    }

    #[test]
    fn test_extract_rss_articles_from_xml_missing_link() {
        // xml(リンク欠落)->channel->rss_articleの流れの確認
        let xml_missing_link = r#"
            <rss version="2.0">
                <channel>
                    <title>Test Feed</title>
                    <item>
                        <title>No Link Article</title>
                    </item>
                    <item>
                        <title>Article With Link</title>
                        <link>http://example.com/with-link</link>
                    </item>
                </channel>
            </rss>
            "#;

        let channel = parse_channel_from_xml(xml_missing_link).expect("Failed to parse test RSS");
        let articles = extract_rss_articles_from_channel(&channel);

        assert_eq!(articles.len(), 1, "リンクがない記事は除外されるはず");
        assert_eq!(articles[0].title, "Article With Link");
    }

    #[test]
    fn test_extract_rss_articles_from_files() {
        // 複数の実際のRSSファイルからリンクを抽出するテスト
        let test_feeds = [
            ("mock/rss/bbc.rss", "BBC"),
            ("mock/rss/cbs.rss", "CBS"),
            ("mock/rss/guardian.rss", "Guardian"),
        ];

        for (file_path, feed_name) in &test_feeds {
            let result = read_channel_from_file(file_path);
            assert!(result.is_ok(), "{}のRSSファイル読み込みに失敗", feed_name);

            let channel = result.unwrap();
            let articles = extract_rss_articles_from_channel(&channel);
            assert!(!articles.is_empty(), "{}の記事が0件", feed_name);

            validate_articles(&articles);
            println!("{}テスト結果: {}件の記事を抽出", feed_name, articles.len());
        }
    }

    #[test]
    fn test_read_non_existing_file() {
        // 存在しないファイルを読み込もうとするテスト
        let result = read_channel_from_file("non_existent_file.rss");
        assert!(result.is_err(), "存在しないファイルでエラーにならなかった");
    }

    // データベース保存機能のテスト
    
    // テスト例1: 基本的な保存機能のテスト
    #[sqlx::test]
    async fn test_save_articles_to_db(pool: PgPool) -> sqlx::Result<()> {
        // テスト用記事データを作成
        let test_articles = vec![
            RssArticle {
                title: "Test Article 1".to_string(),
                link: "https://test.example.com/article1".to_string(),
                description: Some("Test description 1".to_string()),
                pub_date: Some("2025-08-26T10:00:00Z".to_string()),
            },
            RssArticle {
                title: "Test Article 2".to_string(),
                link: "https://test.example.com/article2".to_string(),
                description: Some("Test description 2".to_string()),
                pub_date: Some("2025-08-26T11:00:00Z".to_string()),
            },
        ];

        // データベースに保存をテスト
        let result = save_rss_articles_with_pool(&test_articles, &pool).await?;

        // SaveResultの検証
        assert_eq!(result.inserted, 2, "新規挿入された記事数が期待と異なります");
        assert_eq!(result.skipped, 0, "重複スキップ数が期待と異なります");

        // 実際にデータベースに保存されたことを確認
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rss_articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 2, "期待する件数(2件)が保存されませんでした");

        println!("✅ RSS保存件数検証成功: {}件", result.inserted);
        println!("✅ RSS SaveResult検証成功: {}", result);

        Ok(())
    }

    // テスト例2: 重複記事の処理テスト
    #[sqlx::test(fixtures("duplicate_articles"))]
    async fn test_duplicate_articles(pool: PgPool) -> sqlx::Result<()> {
        // fixtureで既に1件のデータが存在している状態

        // 同じリンクの記事を作成（重複）
        let duplicate_article = RssArticle {
            title: "異なるタイトル".to_string(),
            link: "https://test.example.com/duplicate".to_string(), // fixtureと同じリンク
            description: Some("重複テストの記事".to_string()),
            pub_date: Some("2025-08-26T13:00:00Z".to_string()),
        };

        // 重複記事を保存しようとする
        let result = save_rss_articles_with_pool(&[duplicate_article], &pool).await?;

        // SaveResultの検証
        assert_eq!(
            result.inserted, 0,
            "重複記事が新規挿入されるべきではありません"
        );
        assert_eq!(result.skipped, 1, "重複スキップ数が期待と異なります");

        // データベースの件数は変わらない
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rss_articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 1, "重複記事が挿入され、件数が変わってしまいました");

        println!("✅ RSS重複スキップ検証成功: {}", result);

        Ok(())
    }

    // テスト例3: 空の配列のテスト
    #[sqlx::test]
    async fn test_empty_articles(pool: PgPool) -> sqlx::Result<()> {
        let empty_articles: Vec<RssArticle> = vec![];
        let result = save_rss_articles_with_pool(&empty_articles, &pool).await?;

        // 空配列の結果検証
        assert_eq!(result.inserted, 0, "空配列の新規挿入数は0であるべきです");
        assert_eq!(result.skipped, 0, "空配列の重複スキップ数は0であるべきです");

        // データベースには何も挿入されていない
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rss_articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 0, "空配列でもデータが挿入されてしまいました");

        println!("✅ RSS空配列処理検証成功: {}", result);

        Ok(())
    }

    // テスト例4: 既存データと新規データが混在した場合のテスト
    #[sqlx::test(fixtures("test_articles"))]
    async fn test_mixed_new_and_existing_articles(pool: PgPool) -> sqlx::Result<()> {
        // fixtureで既に2件のデータが存在している状態

        // 1件は既存（重複）、1件は新規のデータを作成
        let mixed_articles = vec![
            RssArticle {
                title: "既存記事".to_string(),
                link: "https://test.example.com/article1".to_string(), // fixtureと同じリンク
                description: Some("この記事は既存です".to_string()),
                pub_date: Some("2025-08-26T14:00:00Z".to_string()),
            },
            RssArticle {
                title: "新規記事".to_string(),
                link: "https://test.example.com/new-article".to_string(), // 新しいリンク
                description: Some("この記事は新規です".to_string()),
                pub_date: Some("2025-08-26T15:00:00Z".to_string()),
            },
        ];

        let result = save_rss_articles_with_pool(&mixed_articles, &pool).await?;

        // SaveResultの検証
        assert_eq!(result.inserted, 1, "新規記事1件が挿入されるべきです");
        assert_eq!(result.skipped, 1, "既存記事1件がスキップされるべきです");

        // 最終的にデータベースには3件（fixture 2件 + 新規 1件）
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rss_articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 3, "期待する件数(3件)と異なります");

        println!("✅ RSS混在データ処理検証成功: {}", result);

        Ok(())
    }
}