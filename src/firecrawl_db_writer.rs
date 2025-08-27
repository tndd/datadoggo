use crate::db_common::{create_pool, initialize_database, SaveResult};
use crate::firecrawl_reader::FirecrawlArticle;
use sqlx::{Error as SqlxError, PgPool};

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
    use std::collections::HashMap;

    // テスト例1: Firecrawl記事の基本的な保存機能のテスト
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
            additional_fields: HashMap::new(),
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

    // テスト例2: Firecrawl記事の重複処理テスト
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
            additional_fields: HashMap::new(),
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