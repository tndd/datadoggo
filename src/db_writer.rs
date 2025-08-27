use crate::rss_reader::RssArticle;
use crate::firecrawl_reader::FirecrawlArticle;
use sqlx::{Error as SqlxError, PgPool};
use std::env;
use std::fmt;

/// データベースへの保存結果を格納する構造体
#[derive(Debug)]
pub struct SaveResult {
    pub inserted: usize,    // 新規にデータベースに挿入された記事
    pub skipped: usize,     // 重複によりスキップされた記事数
}

impl fmt::Display for SaveResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "処理完了: 新規保存{}件、重複スキップ{}件",
            self.inserted, self.skipped
        )
    }
}

/// データベース接続プールを作成
/// .envファイルからDATABASE_URLを読み込みます
async fn create_pool() -> Result<PgPool, SqlxError> {
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL environment variable must be set. Please check your .env file.");
    PgPool::connect(&database_url).await
}

/// データベースの初期化（マイグレーション実行）
async fn initialize_database(pool: &PgPool) -> Result<(), SqlxError> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(SqlxError::from)
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

/// # 概要
/// FirecrawlArticleをデータベースに保存する。
///
/// ## 動作
/// - 自動でデータベース接続プールを作成
/// - マイグレーションを実行
/// - Firecrawl記事を保存
/// - 重複記事（同じURL）は保存をスキップ
///
/// ## 引数
/// - `article`: 保存するFirecrawl記事
///
/// ## 戻り値
/// 成功時は`SaveResult`構造体を返し、保存結果の詳細情報を提供する。
///
/// ## エラー
/// 操作失敗時にはSqlxErrorを返し、全ての操作をロールバックする。
pub async fn save_firecrawl_article_to_db(article: &FirecrawlArticle) -> Result<SaveResult, SqlxError> {
    let pool = create_pool().await?;
    initialize_database(&pool).await?;
    save_firecrawl_article_with_pool(article, &pool).await
}

/// # 概要
/// FirecrawlArticleを指定されたデータベースプールに保存する。
/// 既にプールを準備している場合は `save_firecrawl_article_to_db` ではなく、この関数を使用する。
/// 
/// # Note
/// sqlxの推奨パターンに従い、sqlx::query!マクロを使用してコンパイル時安全性を確保しています。
pub async fn save_firecrawl_article_with_pool(
    article: &FirecrawlArticle,
    pool: &PgPool,
) -> Result<SaveResult, SqlxError> {
    let mut tx = pool.begin().await?;
    
    // メタデータをJSONに変換
    let metadata_json = serde_json::to_value(&article.metadata)
        .map_err(|e| SqlxError::Decode(Box::new(e)))?;
    
    // URLを取得（存在しない場合はデフォルト値を使用）
    let url = article.metadata.url.as_deref()
        .or(article.metadata.source_url.as_deref())
        .unwrap_or("unknown");
    
    // タイトルを取得
    let title = article.metadata.title.as_deref()
        .or(article.metadata.og_title.as_deref())
        .or(article.metadata.og_title_alt.as_deref());
    
    // cached_atを解析してTimestamp用の値を作成
    let scraped_at_str = article.metadata.cached_at.as_deref();

    let result = sqlx::query!(
        r#"
        INSERT INTO firecrawl_articles (url, title, markdown_content, metadata_json, scraped_at)
        VALUES ($1, $2, $3, $4, $5::text::timestamp)
        ON CONFLICT (url) DO NOTHING
        "#,
        url,
        title,
        article.markdown,
        metadata_json,
        scraped_at_str
    )
    .execute(&mut *tx)
    .await?;
    
    let inserted = if result.rows_affected() > 0 { 1 } else { 0 };
    
    tx.commit().await?;

    Ok(SaveResult {
        inserted,
        skipped: 1 - inserted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::firecrawl_reader::*;

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

        println!("✅ 保存件数検証成功: {}件", result.inserted);
        println!("✅ SaveResult検証成功: {}", result);

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

        println!("✅ 重複スキップ検証成功: {}", result);

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

        println!("✅ 空配列処理検証成功: {}", result);

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

        println!("✅ 混在データ処理検証成功: {}", result);

        Ok(())
    }

    // テスト例5: Firecrawl記事の基本的な保存機能のテスト
    #[sqlx::test]
    async fn test_save_firecrawl_article_to_db(pool: PgPool) -> sqlx::Result<()> {
        // テスト用のFirecrawl記事データを作成
        let metadata = FirecrawlMetadata {
            favicon: None,
            page_section: None,
            viewport: None,
            og_image_alt: None,
            theme_color: None,
            title: Some("Test Firecrawl Article".to_string()),
            al_android_package: None,
            page_subsection: None,
            og_title: None,
            next_head_count: None,
            al_ios_app_store_id: None,
            og_image: None,
            og_description: None,
            og_description_alt: None,
            robots: None,
            og_image_alt_field: None,
            twitter_image_src: None,
            al_android_app_name: None,
            description: Some("Test description for firecrawl".to_string()),
            al_ios_app_name: None,
            language: None,
            msapplication_tile_color: None,
            al_web_url: None,
            article_modified_time: None,
            cxense_parse_publishtime: None,
            cxense_parse_author: None,
            twitter_card: None,
            google_site_verification: None,
            color_scheme: None,
            twitter_description: None,
            version: None,
            og_title_alt: None,
            al_ios_url: None,
            twitter_image_alt: None,
            cxense_parse_pageclass: None,
            al_android_url: None,
            apple_itunes_app: None,
            twitter_title: None,
            scrape_id: None,
            source_url: None,
            url: Some("https://test.example.com/firecrawl".to_string()),
            status_code: Some(200),
            content_type: Some("text/html".to_string()),
            proxy_used: None,
            cache_state: None,
            cached_at: Some("2025-08-27T10:00:00Z".to_string()),
            credits_used: Some(1),
            additional_fields: std::collections::HashMap::new(),
        };

        let test_article = FirecrawlArticle {
            markdown: "# Test Article\n\nThis is a test markdown content.".to_string(),
            metadata,
        };

        // データベースに保存をテスト
        let result = save_firecrawl_article_with_pool(&test_article, &pool).await?;

        // SaveResultの検証
        assert_eq!(result.inserted, 1, "新規挿入された記事数が期待と異なります");
        assert_eq!(result.skipped, 0, "重複スキップ数が期待と異なります");

        // 実際にデータベースに保存されたことを確認
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM firecrawl_articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 1, "期待する件数(1件)が保存されませんでした");

        println!("✅ Firecrawl記事保存件数検証成功: {}件", result.inserted);
        println!("✅ Firecrawl SaveResult検証成功: {}", result);

        Ok(())
    }

    // テスト例6: Firecrawl記事の重複処理テスト
    #[sqlx::test]
    async fn test_duplicate_firecrawl_articles(pool: PgPool) -> sqlx::Result<()> {
        // 最初の記事を保存
        let mut metadata = FirecrawlMetadata {
            favicon: None,
            page_section: None,
            viewport: None,
            og_image_alt: None,
            theme_color: None,
            title: Some("Original Article".to_string()),
            al_android_package: None,
            page_subsection: None,
            og_title: None,
            next_head_count: None,
            al_ios_app_store_id: None,
            og_image: None,
            og_description: None,
            og_description_alt: None,
            robots: None,
            og_image_alt_field: None,
            twitter_image_src: None,
            al_android_app_name: None,
            description: None,
            al_ios_app_name: None,
            language: None,
            msapplication_tile_color: None,
            al_web_url: None,
            article_modified_time: None,
            cxense_parse_publishtime: None,
            cxense_parse_author: None,
            twitter_card: None,
            google_site_verification: None,
            color_scheme: None,
            twitter_description: None,
            version: None,
            og_title_alt: None,
            al_ios_url: None,
            twitter_image_alt: None,
            cxense_parse_pageclass: None,
            al_android_url: None,
            apple_itunes_app: None,
            twitter_title: None,
            scrape_id: None,
            source_url: None,
            url: Some("https://test.example.com/duplicate".to_string()),
            status_code: Some(200),
            content_type: None,
            proxy_used: None,
            cache_state: None,
            cached_at: None,
            credits_used: None,
            additional_fields: std::collections::HashMap::new(),
        };

        let original_article = FirecrawlArticle {
            markdown: "Original content".to_string(),
            metadata: metadata.clone(),
        };

        // 最初の記事を保存
        let result1 = save_firecrawl_article_with_pool(&original_article, &pool).await?;
        assert_eq!(result1.inserted, 1);

        // 同じURLで違うタイトルの記事を作成（重複）
        metadata.title = Some("Different Title".to_string());
        let duplicate_article = FirecrawlArticle {
            markdown: "Different content".to_string(),
            metadata,
        };

        // 重複記事を保存しようとする
        let result2 = save_firecrawl_article_with_pool(&duplicate_article, &pool).await?;

        // SaveResultの検証
        assert_eq!(result2.inserted, 0, "重複記事が新規挿入されるべきではありません");
        assert_eq!(result2.skipped, 1, "重複スキップ数が期待と異なります");

        // データベースの件数は1件のまま
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM firecrawl_articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 1, "重複記事が挿入され、件数が変わってしまいました");

        println!("✅ Firecrawl重複スキップ検証成功: {}", result2);

        Ok(())
    }
}
