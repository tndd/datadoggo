use crate::infra::db::setup_database;
use crate::infra::loader::load_file;
use crate::types::DatabaseInsertResult;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;


/// Firecrawl操作の結果型（DatabaseInsertResultの型エイリアス）
pub type FirecrawlOperationResult = DatabaseInsertResult;


// Firecrawl記事の情報を格納する構造体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirecrawlArticle {
    pub markdown: String,
    pub metadata: FirecrawlMetadata,
}

// Firecrawlのメタデータを格納する構造体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirecrawlMetadata {
    pub favicon: Option<String>,
    #[serde(rename = "page.section")]
    pub page_section: Option<String>,
    pub viewport: Option<Vec<String>>,
    #[serde(rename = "og:image:alt")]
    pub og_image_alt: Option<String>,
    #[serde(rename = "theme-color")]
    pub theme_color: Option<Vec<String>>,
    pub title: Option<String>,
    #[serde(rename = "al:android:package")]
    pub al_android_package: Option<String>,
    #[serde(rename = "page.subsection")]
    pub page_subsection: Option<String>,
    #[serde(rename = "ogTitle")]
    pub og_title: Option<String>,
    #[serde(rename = "next-head-count")]
    pub next_head_count: Option<String>,
    #[serde(rename = "al:ios:app_store_id")]
    pub al_ios_app_store_id: Option<String>,
    #[serde(rename = "og:image")]
    pub og_image: Option<String>,
    #[serde(rename = "og:description")]
    pub og_description: Option<String>,
    #[serde(rename = "ogDescription")]
    pub og_description_alt: Option<String>,
    pub robots: Option<String>,
    #[serde(rename = "ogImage")]
    pub og_image_alt_field: Option<String>,
    #[serde(rename = "twitter:image:src")]
    pub twitter_image_src: Option<String>,
    #[serde(rename = "al:android:app_name")]
    pub al_android_app_name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "al:ios:app_name")]
    pub al_ios_app_name: Option<String>,
    pub language: Option<String>,
    #[serde(rename = "msapplication-TileColor")]
    pub msapplication_tile_color: Option<String>,
    #[serde(rename = "al:web:url")]
    pub al_web_url: Option<String>,
    #[serde(rename = "article:modified_time")]
    pub article_modified_time: Option<String>,
    #[serde(rename = "cXenseParse:publishtime")]
    pub cxense_parse_publishtime: Option<String>,
    #[serde(rename = "cXenseParse:author")]
    pub cxense_parse_author: Option<String>,
    #[serde(rename = "twitter:card")]
    pub twitter_card: Option<String>,
    #[serde(rename = "google-site-verification")]
    pub google_site_verification: Option<String>,
    #[serde(rename = "color-scheme")]
    pub color_scheme: Option<String>,
    #[serde(rename = "twitter:description")]
    pub twitter_description: Option<String>,
    pub version: Option<String>,
    #[serde(rename = "og:title")]
    pub og_title_alt: Option<String>,
    #[serde(rename = "al:ios:url")]
    pub al_ios_url: Option<String>,
    #[serde(rename = "twitter:image:alt")]
    pub twitter_image_alt: Option<String>,
    #[serde(rename = "cXenseParse:pageclass")]
    pub cxense_parse_pageclass: Option<String>,
    #[serde(rename = "al:android:url")]
    pub al_android_url: Option<String>,
    #[serde(rename = "apple-itunes-app")]
    pub apple_itunes_app: Option<String>,
    #[serde(rename = "twitter:title")]
    pub twitter_title: Option<String>,
    #[serde(rename = "scrapeId")]
    pub scrape_id: Option<String>,
    #[serde(rename = "sourceURL")]
    pub source_url: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "statusCode")]
    pub status_code: Option<i32>,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
    #[serde(rename = "proxyUsed")]
    pub proxy_used: Option<String>,
    #[serde(rename = "cacheState")]
    pub cache_state: Option<String>,
    #[serde(rename = "cachedAt")]
    pub cached_at: Option<String>,
    #[serde(rename = "creditsUsed")]
    pub credits_used: Option<i32>,

    // その他のフィールドをキャッチするため
    #[serde(flatten)]
    pub additional_fields: HashMap<String, serde_json::Value>,
}

// ファイルからFirecrawlデータを読み込むヘルパー関数（loaderを使用）
pub fn read_firecrawl_from_file(file_path: &str) -> Result<FirecrawlArticle> {
    let buf_reader = load_file(file_path)?;
    let article: FirecrawlArticle = serde_json::from_reader(buf_reader)
        .with_context(|| format!("Firecrawlファイルの解析に失敗: {}", file_path))?;
    Ok(article)
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
/// 操作失敗時には全ての操作をロールバックする。
pub async fn save_firecrawl_article_to_db(
    article: &FirecrawlArticle,
) -> Result<FirecrawlOperationResult> {
    let pool = setup_database().await?;
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
) -> Result<FirecrawlOperationResult> {
    let mut tx = pool
        .begin()
        .await
        .context("トランザクションの開始に失敗しました")?;

    // メタデータをJSONに変換
    let metadata_json = serde_json::to_value(&article.metadata)
        .context("メタデータのJSONシリアライズに失敗しました")?;

    // URLを取得（存在しない場合はデフォルト値を使用）
    let url = article
        .metadata
        .url
        .as_deref()
        .or(article.metadata.source_url.as_deref())
        .unwrap_or("unknown");

    // タイトルを取得
    let title = article
        .metadata
        .title
        .as_deref()
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
    .await
    .context("Firecrawl記事のデータベースへの挿入に失敗しました")?;

    let inserted = if result.rows_affected() > 0 { 1 } else { 0 };

    tx.commit()
        .await
        .context("トランザクションのコミットに失敗しました")?;

    Ok(FirecrawlOperationResult::new(inserted, 1 - inserted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_firecrawl_from_file() {
        // BBCのモックファイルを読み込んでパース
        let result = read_firecrawl_from_file("mock/fc/bbc.json");
        assert!(result.is_ok(), "Firecrawl JSONファイルの読み込みに失敗");

        let article = result.unwrap();

        // 基本的なフィールドの検証
        assert!(!article.markdown.is_empty(), "markdownが空です");
        assert!(article.metadata.title.is_some(), "タイトルがありません");
        assert!(article.metadata.url.is_some(), "URLがありません");

        println!("✅ Firecrawlデータの読み込みテスト成功");
        println!("タイトル: {:?}", article.metadata.title);
        println!("URL: {:?}", article.metadata.url);
        println!("Markdownサイズ: {} characters", article.markdown.len());
    }

    #[test]
    fn test_read_non_existing_file() {
        // 存在しないファイルを読み込もうとするテスト
        let result = read_firecrawl_from_file("non_existent_file.json");
        assert!(result.is_err(), "存在しないファイルでエラーにならなかった");
    }

    // データベース保存機能のテスト

    // テスト例1: Firecrawl記事の基本的な保存機能のテスト
    #[sqlx::test]
    async fn test_save_firecrawl_article_to_db(
        pool: PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
        assert_eq!(result.skipped_duplicate, 0, "重複スキップ数が期待と異なります");

        // 実際にデータベースに保存されたことを確認
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM firecrawl_articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 1, "期待する件数(1件)が保存されませんでした");

        println!("✅ Firecrawl記事保存件数検証成功: {}件", result.inserted);
        println!("✅ Firecrawl SaveResult検証成功: {}", result.display_with_domain("Firecrawlドキュメント"));

        Ok(())
    }

    // テスト例2: Firecrawl記事の重複処理テスト
    #[sqlx::test]
    async fn test_duplicate_firecrawl_articles(
        pool: PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
        assert_eq!(
            result2.inserted, 0,
            "重複記事が新規挿入されるべきではありません"
        );
        assert_eq!(result2.skipped_duplicate, 1, "重複スキップ数が期待と異なります");

        // データベースの件数は1件のまま
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM firecrawl_articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 1, "重複記事が挿入され、件数が変わってしまいました");

        println!("✅ Firecrawl重複スキップ検証成功: {}", result2);

        Ok(())
    }
}
