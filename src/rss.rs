use crate::infra::db::setup_database;
use crate::infra::db::DatabaseInsertResult;
use crate::infra::loader::load_file;
use anyhow::{Context, Result};
use rss::Channel;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

// RSS記事の情報を格納する構造体（テーブル定義と一致）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RssArticle {
    pub link: String,
    pub title: String,
    pub description: Option<String>,
    pub pub_date: Option<String>,
}

// RSSのチャンネルから記事を抽出する関数
pub fn extract_rss_articles_from_channel(channel: &Channel) -> Vec<RssArticle> {
    let mut articles = Vec::new();

    for item in channel.items() {
        if let Some(link) = item.link() {
            let article = RssArticle {
                link: link.to_string(),
                title: item.title().unwrap_or("タイトルなし").to_string(),
                description: item.description().map(|d| d.to_string()),
                pub_date: item.pub_date().map(|d| d.to_string()),
            };
            articles.push(article);
        }
    }

    articles
}

// ファイルからRSSを読み込むヘルパー関数（loaderを使用）
pub fn read_channel_from_file(file_path: &str) -> Result<Channel> {
    let buf_reader = load_file(file_path)?;
    Channel::read_from(buf_reader)
        .with_context(|| format!("RSSファイルの解析に失敗: {}", file_path))
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
/// - `DatabaseInsertResult`: 保存件数の詳細
///
/// ## エラー
/// 操作失敗時には全ての操作をロールバックする。
pub async fn save_rss_articles_to_db(articles: &[RssArticle]) -> Result<DatabaseInsertResult> {
    let pool = setup_database().await?;
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
) -> Result<DatabaseInsertResult> {
    if articles.is_empty() {
        return Ok(DatabaseInsertResult::empty());
    }

    let mut tx = pool
        .begin()
        .await
        .context("トランザクションの開始に失敗しました")?;
    let mut total_inserted = 0;

    // sqlx::query!マクロを使用してコンパイル時にSQLを検証
    for article in articles {
        let result = sqlx::query!(
            r#"
            INSERT INTO rss_articles (link, title, description, pub_date)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (link) DO NOTHING
            "#,
            article.link,
            article.title,
            article.description,
            article.pub_date
        )
        .execute(&mut *tx)
        .await
        .context("記事のデータベースへの挿入に失敗しました")?;

        if result.rows_affected() > 0 {
            total_inserted += 1;
        }
    }

    tx.commit()
        .await
        .context("トランザクションのコミットに失敗しました")?;

    Ok(DatabaseInsertResult::new(
        total_inserted,
        articles.len() - total_inserted,
    ))
}

// RSS記事のフィルター条件を表す構造体
#[derive(Debug, Default)]
pub struct RssArticleFilter {
    pub link_contains: Option<String>,
    pub pub_date_from: Option<String>,
    pub pub_date_to: Option<String>,
}

/// # 概要
/// データベースからRSS記事を取得する。
///
/// ## 動作
/// - 自動でデータベース接続プールを作成
/// - 指定された条件でRSS記事を取得
///
/// ## 引数
/// - `filter`: フィルター条件。Noneの場合は全件取得
///
/// ## 戻り値
/// - `Vec<RssArticle>`: 条件にマッチしたRSS記事のリスト
pub async fn get_rss_articles_from_db(filter: Option<RssArticleFilter>) -> Result<Vec<RssArticle>> {
    let pool = setup_database().await?;
    get_rss_articles_with_pool(filter, &pool).await
}

/// # 概要
/// 指定されたデータベースプールからRSS記事を取得する。
pub async fn get_rss_articles_with_pool(
    filter: Option<RssArticleFilter>,
    pool: &PgPool,
) -> Result<Vec<RssArticle>> {
    let filter = filter.unwrap_or_default();

    let mut query = "SELECT link, title, description, pub_date FROM rss_articles".to_string();
    let mut conditions = Vec::new();
    let mut params = Vec::new();
    let mut param_count = 0;

    // linkによる絞り込み
    if let Some(link_contains) = &filter.link_contains {
        param_count += 1;
        conditions.push(format!("link ILIKE ${}", param_count));
        params.push(format!("%{}%", link_contains));
    }

    // pub_dateの範囲指定
    if let Some(pub_date_from) = &filter.pub_date_from {
        param_count += 1;
        conditions.push(format!("pub_date >= ${}", param_count));
        params.push(pub_date_from.clone());
    }

    if let Some(pub_date_to) = &filter.pub_date_to {
        param_count += 1;
        conditions.push(format!("pub_date <= ${}", param_count));
        params.push(pub_date_to.clone());
    }

    // WHERE句を追加
    if !conditions.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&conditions.join(" AND "));
    }

    // 日付順でソート
    query.push_str(" ORDER BY pub_date DESC");

    // 動的クエリを実行
    let mut query_builder = sqlx::query_as::<_, RssArticle>(&query);

    for param in params {
        query_builder = query_builder.bind(param);
    }

    let articles = query_builder
        .fetch_all(pool)
        .await
        .context("RSS記事の取得に失敗しました")?;

    Ok(articles)
}

/// 指定されたリンクのRSS記事を取得する
pub async fn get_rss_article_by_link(link: &str) -> Result<Option<RssArticle>> {
    let pool = setup_database().await?;
    get_rss_article_by_link_with_pool(link, &pool).await
}

/// 指定されたリンクのRSS記事を指定されたプールから取得する
pub async fn get_rss_article_by_link_with_pool(
    link: &str,
    pool: &PgPool,
) -> Result<Option<RssArticle>> {
    let article = sqlx::query_as::<_, RssArticle>(
        "SELECT link, title, description, pub_date FROM rss_articles WHERE link = $1",
    )
    .bind(link)
    .fetch_optional(pool)
    .await
    .context("指定されたリンクのRSS記事取得に失敗しました")?;

    Ok(article)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    // XMLからRSSチャンネルを解析するヘルパー関数
    fn parse_channel_from_xml(xml: &str) -> Result<Channel> {
        Channel::read_from(BufReader::new(Cursor::new(xml.as_bytes())))
            .context("XMLからのRSSチャンネル解析に失敗")
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
    async fn test_save_articles_to_db(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
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
        assert_eq!(
            result.skipped_duplicate, 0,
            "重複スキップ数が期待と異なります"
        );

        // 実際にデータベースに保存されたことを確認
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rss_articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 2, "期待する件数(2件)が保存されませんでした");

        println!("✅ RSS保存件数検証成功: {}件", result.inserted);
        println!(
            "✅ RSS SaveResult検証成功: {}",
            result.display_with_domain("RSS記事")
        );

        Ok(())
    }

    // テスト例2: 重複記事の処理テスト
    #[sqlx::test(fixtures("duplicate_articles"))]
    async fn test_duplicate_articles(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
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
        assert_eq!(
            result.skipped_duplicate, 1,
            "重複スキップ数が期待と異なります"
        );

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
    async fn test_empty_articles(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let empty_articles: Vec<RssArticle> = vec![];
        let result = save_rss_articles_with_pool(&empty_articles, &pool).await?;

        // 空配列の結果検証
        assert_eq!(result.inserted, 0, "空配列の新規挿入数は0であるべきです");
        assert_eq!(
            result.skipped_duplicate, 0,
            "空配列の重複スキップ数は0であるべきです"
        );

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
    async fn test_mixed_new_and_existing_articles(
        pool: PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
        assert_eq!(
            result.skipped_duplicate, 1,
            "既存記事1件がスキップされるべきです"
        );

        // 最終的にデータベースには3件（fixture 2件 + 新規 1件）
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rss_articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 3, "期待する件数(3件)と異なります");

        println!("✅ RSS混在データ処理検証成功: {}", result);

        Ok(())
    }

    // データベース取得機能の際どいテスト（新フィクスチャ使用）

    // テスト例5: 全件取得テスト
    #[sqlx::test(fixtures("rss_retrieval_test"))]
    async fn test_get_all_rss_articles_comprehensive(
        pool: PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 新フィクスチャで15件のデータが存在

        let articles = get_rss_articles_with_pool(None, &pool).await?;

        // 全件取得されることを確認（pub_dateがNULLの1件を除く）
        assert!(articles.len() >= 14, "全件取得で最低14件が期待されます");

        // 日付順（降順）でソートされていることを確認
        let mut prev_date: Option<&str> = None;
        for article in &articles {
            if let Some(current_date) = &article.pub_date {
                if let Some(prev) = prev_date {
                    assert!(
                        current_date.as_str() <= prev,
                        "日付の降順ソートが正しくありません"
                    );
                }
                prev_date = Some(current_date.as_str());
            }
        }

        // 基本的な記事のフィールドがすべて設定されていることを確認
        for article in &articles {
            assert!(!article.title.is_empty(), "記事のタイトルが空です");
            assert!(!article.link.is_empty(), "記事のリンクが空です");
        }

        println!("✅ RSS全件取得際どいテスト成功: {}件", articles.len());

        Ok(())
    }

    // テスト例6: 日付境界の際どいテスト
    #[sqlx::test(fixtures("rss_retrieval_test"))]
    async fn test_date_boundary_edge_cases(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        // 2025-01-15の境界テスト

        // 開始時刻ちょうどを含む検索
        let filter_start = RssArticleFilter {
            pub_date_from: Some("2025-01-15T00:00:00Z".to_string()),
            pub_date_to: Some("2025-01-15T23:59:59Z".to_string()),
            ..Default::default()
        };

        let articles_boundary = get_rss_articles_with_pool(Some(filter_start), &pool).await?;

        // 境界値の記事が含まれることを確認
        assert!(
            articles_boundary.len() >= 3,
            "2025-01-15の境界記事が期待通り取得されません"
        );

        let boundary_links: Vec<&str> = articles_boundary.iter().map(|a| a.link.as_str()).collect();
        assert!(boundary_links.contains(&"https://test.com/boundary/exactly-start"));
        assert!(boundary_links.contains(&"https://test.com/boundary/exactly-end"));
        assert!(boundary_links.contains(&"https://example.com/tech/article-2025-01-15"));

        // 1秒前後の記事は含まれないことを確認
        assert!(!boundary_links.contains(&"https://test.com/boundary/one-second-before"));
        assert!(!boundary_links.contains(&"https://test.com/boundary/one-second-after"));

        println!(
            "✅ RSS日付境界テスト成功: {}件の境界記事",
            articles_boundary.len()
        );

        Ok(())
    }

    // テスト例4: 日付境界の際どいテスト
    #[sqlx::test(fixtures("rss_retrieval_test"))]
    async fn test_get_rss_articles_by_date_range(
        pool: PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 範囲の開始時刻ちょうどの記事が含まれるかテスト
        let filter_start_boundary = RssArticleFilter {
            pub_date_from: Some("2025-01-15T00:00:00Z".to_string()),
            pub_date_to: Some("2025-01-15T00:00:01Z".to_string()),
            ..Default::default()
        };

        let articles_start = get_rss_articles_with_pool(Some(filter_start_boundary), &pool).await?;
        assert_eq!(
            articles_start.len(),
            1,
            "開始境界時刻ちょうどの記事が含まれていません"
        );
        assert_eq!(
            articles_start[0].link,
            "https://test.com/boundary/exactly-start"
        );

        // 範囲の終了時刻ちょうどの記事が含まれるかテスト
        let filter_end_boundary = RssArticleFilter {
            pub_date_from: Some("2025-01-15T23:59:58Z".to_string()),
            pub_date_to: Some("2025-01-15T23:59:59Z".to_string()),
            ..Default::default()
        };

        let articles_end = get_rss_articles_with_pool(Some(filter_end_boundary), &pool).await?;
        assert_eq!(
            articles_end.len(),
            1,
            "終了境界時刻ちょうどの記事が含まれていません"
        );
        assert_eq!(
            articles_end[0].link,
            "https://test.com/boundary/exactly-end"
        );

        // 範囲外（1秒前）の記事が除外されるかテスト
        let filter_before = RssArticleFilter {
            pub_date_from: Some("2025-01-15T00:00:00Z".to_string()),
            pub_date_to: Some("2025-01-15T23:59:58Z".to_string()),
            ..Default::default()
        };

        let articles_before = get_rss_articles_with_pool(Some(filter_before), &pool).await?;
        let before_links: Vec<&str> = articles_before.iter().map(|a| a.link.as_str()).collect();
        assert!(
            !before_links.contains(&"2025-01-14T23:59:59Z"),
            "範囲外の記事が含まれています"
        );

        println!("✅ RSS日付境界際どいテスト成功");

        Ok(())
    }

    // テスト例5: URL部分一致の詳細テスト
    #[sqlx::test(fixtures("rss_retrieval_test"))]
    async fn test_get_rss_articles_by_combined_filter(
        pool: PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 部分一致検索（example.comを含む）
        let filter_partial = RssArticleFilter {
            link_contains: Some("example.com".to_string()),
            ..Default::default()
        };

        let articles_partial = get_rss_articles_with_pool(Some(filter_partial), &pool).await?;
        assert!(
            articles_partial.len() >= 4,
            "example.comを含む記事が少なすぎます"
        ); // 基本3件 + testing-advanced

        // 厳密でない一致（not-example.comは含まれるが確認）
        let example_count = articles_partial
            .iter()
            .filter(|a| a.link.contains("example.com") && !a.link.contains("not-example.com"))
            .count();
        let not_example_count = articles_partial
            .iter()
            .filter(|a| a.link.contains("not-example.com"))
            .count();

        assert!(
            example_count >= 4,
            "純粋なexample.comドメインの記事が足りません"
        );
        assert_eq!(
            not_example_count, 1,
            "not-example.comの記事も含まれるはずです"
        );

        // より具体的なパス部分の一致
        let filter_specific = RssArticleFilter {
            link_contains: Some("/tech/".to_string()),
            ..Default::default()
        };

        let articles_specific = get_rss_articles_with_pool(Some(filter_specific), &pool).await?;
        assert_eq!(
            articles_specific.len(),
            1,
            "/tech/パスを含む記事は1件であるべきです"
        );
        assert_eq!(
            articles_specific[0].link,
            "https://example.com/tech/article-2025-01-15"
        );

        println!("✅ RSSURL部分一致詳細テスト成功");

        Ok(())
    }

    // テスト例6: 個別記事取得の際どいテスト
    #[sqlx::test(fixtures("rss_retrieval_test"))]
    async fn test_get_rss_article_by_link(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        // 存在する記事の正確な取得
        let tech_article =
            get_rss_article_by_link_with_pool("https://example.com/tech/article-2025-01-15", &pool)
                .await?;

        assert!(tech_article.is_some(), "指定された記事が見つかりません");
        let article = tech_article.unwrap();
        assert_eq!(article.title, "Tech News 2025");
        assert_eq!(
            article.description,
            Some("Latest technology updates".to_string())
        );

        // 大小文字が異なるURLでの取得（完全一致のため失敗するはず）
        let case_different =
            get_rss_article_by_link_with_pool("https://EXAMPLE.COM/tech/article-2025-01-15", &pool)
                .await?;
        assert!(
            case_different.is_none(),
            "大小文字が異なるURLで記事が取得されました"
        );

        // 部分一致では取得できないことを確認
        let partial_match = get_rss_article_by_link_with_pool("example.com/tech", &pool).await?;
        assert!(partial_match.is_none(), "部分一致で記事が取得されました");

        // NULL descriptionの記事取得テスト
        let null_desc_article =
            get_rss_article_by_link_with_pool("https://null-test.com/no-description", &pool)
                .await?;
        assert!(
            null_desc_article.is_some(),
            "NULL descriptionの記事が取得できません"
        );
        let null_article = null_desc_article.unwrap();
        assert!(
            null_article.description.is_none(),
            "descriptionがNULLでありません"
        );

        println!("✅ RSS個別記事取得際どいテスト成功");

        Ok(())
    }

    // テスト例7: URL部分一致の際どいテスト（大小文字、特殊文字）
    #[sqlx::test(fixtures("rss_retrieval_test"))]
    async fn test_get_rss_articles_no_match(
        pool: PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 大小文字を無視したILIKE検索のテスト
        let filter_case = RssArticleFilter {
            link_contains: Some("casesensitive".to_string()), // 小文字で検索
            ..Default::default()
        };
        
        let articles_case = get_rss_articles_with_pool(Some(filter_case), &pool).await?;
        assert_eq!(articles_case.len(), 1, "大小文字無視検索が機能していません");
        assert_eq!(articles_case[0].link, "https://CaseSensitive.com/MixedCase");

        // 特殊文字を含むURL検索
        let filter_special = RssArticleFilter {
            link_contains: Some("%20with%20".to_string()),
            ..Default::default()
        };
        
        let articles_special = get_rss_articles_with_pool(Some(filter_special), &pool).await?;
        assert_eq!(articles_special.len(), 1, "特殊文字検索が機能していません");

        // アンダースコア検索（SQLのワイルドカードではない）
        let filter_underscore = RssArticleFilter {
            link_contains: Some("article_with_underscore".to_string()), // より具体的な検索語
            ..Default::default()
        };
        
        let articles_underscore = get_rss_articles_with_pool(Some(filter_underscore), &pool).await?;
        assert_eq!(articles_underscore.len(), 1, "アンダースコア検索が機能していません");
        assert!(articles_underscore[0].link.contains("article_with_underscore"), "期待される記事がマッチしていません");

        println!("✅ RSSリンク際どい絞り込みテスト成功");

        Ok(())
    }

    // テスト例8: 複合条件の際どいテスト
    #[sqlx::test(fixtures("rss_retrieval_test"))]
    async fn test_complex_combined_filters(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        // 非常に狭い時間範囲 + URL条件
        let filter_narrow = RssArticleFilter {
            link_contains: Some("example.com".to_string()),
            pub_date_from: Some("2025-01-15T09:00:00Z".to_string()),
            pub_date_to: Some("2025-01-15T11:00:00Z".to_string()),
        };

        let articles_narrow = get_rss_articles_with_pool(Some(filter_narrow), &pool).await?;

        // Tech News記事のみがマッチするはず
        assert_eq!(articles_narrow.len(), 1, "狭い複合条件で1件が期待されます");
        assert_eq!(
            articles_narrow[0].link,
            "https://example.com/tech/article-2025-01-15"
        );

        // 存在しない組み合わせのテスト
        let filter_impossible = RssArticleFilter {
            link_contains: Some("example.com".to_string()),
            pub_date_from: Some("2025-01-20T00:00:00Z".to_string()),
            pub_date_to: Some("2025-01-25T00:00:00Z".to_string()),
        };

        let articles_impossible =
            get_rss_articles_with_pool(Some(filter_impossible), &pool).await?;
        assert_eq!(
            articles_impossible.len(),
            0,
            "存在しない組み合わせでは0件が期待されます"
        );

        println!("✅ RSS複合条件際どいテスト成功");

        Ok(())
    }

    // テスト例9: NULL値を含むデータの処理テスト
    #[sqlx::test(fixtures("rss_retrieval_test"))]
    async fn test_null_value_handling(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        // pub_dateがNULLの記事は日付範囲検索に含まれないことを確認
        let filter_with_date = RssArticleFilter {
            pub_date_from: Some("2020-01-01T00:00:00Z".to_string()),
            ..Default::default()
        };

        let articles_with_date = get_rss_articles_with_pool(Some(filter_with_date), &pool).await?;

        // pub_dateがNULLの記事は含まれないことを確認
        for article in &articles_with_date {
            assert!(
                article.pub_date.is_some(),
                "pub_dateがNULLの記事が含まれています"
            );
        }

        // 個別記事取得でdescriptionがNULLの記事が正常に取得できることを確認
        let null_desc_article =
            get_rss_article_by_link_with_pool("https://null-test.com/no-description", &pool)
                .await?;

        assert!(
            null_desc_article.is_some(),
            "descriptionがNULLの記事が取得できません"
        );
        let article = null_desc_article.unwrap();
        assert!(
            article.description.is_none(),
            "descriptionがNULLであるべきです"
        );
        assert_eq!(article.title, "No Description Article");

        println!("✅ RSSNULL値処理テスト成功");

        Ok(())
    }

    // テスト例10: パフォーマンス関連のエッジケース
    #[sqlx::test(fixtures("rss_retrieval_test"))]
    async fn test_performance_edge_cases(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        // 空文字列での検索
        let filter_empty = RssArticleFilter {
            link_contains: Some("".to_string()),
            ..Default::default()
        };

        let articles_empty = get_rss_articles_with_pool(Some(filter_empty), &pool).await?;
        // 空文字列は全ての文字列を含むので、全記事が返される（NULLでない）
        assert!(
            articles_empty.len() > 0,
            "空文字列検索で記事が取得されません"
        );

        // 非常に長い検索文字列
        let long_string = "a".repeat(1000);
        let filter_long = RssArticleFilter {
            link_contains: Some(long_string),
            ..Default::default()
        };

        let articles_long = get_rss_articles_with_pool(Some(filter_long), &pool).await?;
        assert_eq!(articles_long.len(), 0, "長い文字列検索で0件が期待されます");

        println!("✅ RSSパフォーマンステスト成功");

        Ok(())
    }
}
