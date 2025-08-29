use crate::infra::db::setup_database;
use crate::infra::db::DatabaseInsertResult;
use crate::infra::loader::load_file;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;

// Firecrawl記事構造体（テーブル定義と一致）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Article {
    pub url: String,
    pub timestamp: OffsetDateTime,
    pub status_code: Option<i32>,
    pub content: String,
}

// ファイルからFirecrawlデータを読み込み、Articleに変換する
pub fn read_article_from_file(file_path: &str) -> Result<Article> {
    let buf_reader = load_file(file_path)?;
    let json_value: serde_json::Value = serde_json::from_reader(buf_reader)
        .with_context(|| format!("Firecrawlファイルの解析に失敗: {}", file_path))?;

    // JSONから必要な値を抽出
    let content = json_value
        .get("markdown")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("markdownフィールドが見つかりません"))?
        .to_string();

    let metadata = json_value
        .get("metadata")
        .ok_or_else(|| anyhow::anyhow!("metadataフィールドが見つかりません"))?;

    // URLを取得（複数の候補から）
    let url = metadata
        .get("url")
        .and_then(|v| v.as_str())
        .or_else(|| metadata.get("sourceURL").and_then(|v| v.as_str()))
        .ok_or_else(|| anyhow::anyhow!("URLが見つかりません"))?
        .to_string();

    // status_codeを取得
    let status_code = metadata
        .get("statusCode")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);

    let now = OffsetDateTime::now_utc();

    Ok(Article {
        url,
        timestamp: now,
        status_code,
        content,
    })
}

/// # 概要
/// Articleをデータベースに保存する。
///
/// ## 動作
/// - 自動でデータベース接続プールを作成
/// - マイグレーションを実行
/// - Firecrawl記事を保存
/// - 重複記事（同じURL）は更新
///
/// ## 引数
/// - `article`: 保存する記事
///
/// ## 戻り値
/// - `DatabaseInsertResult`: 保存件数の詳細
///
/// ## エラー
/// 操作失敗時には全ての操作をロールバックする。
pub async fn save_article_to_db(article: &Article) -> Result<DatabaseInsertResult> {
    let pool = setup_database().await?;
    save_article_with_pool(article, &pool).await
}

/// # 概要
/// Articleを指定されたデータベースプールに保存する。
/// 既にプールを準備している場合は `save_article_to_db` ではなく、この関数を使用する。
///
/// # Note
/// sqlxの推奨パターンに従い、sqlx::query!マクロを使用してコンパイル時安全性を確保しています。
pub async fn save_article_with_pool(
    article: &Article,
    pool: &PgPool,
) -> Result<DatabaseInsertResult> {
    let mut tx = pool
        .begin()
        .await
        .context("トランザクションの開始に失敗しました")?;

    let result = sqlx::query!(
        r#"
        INSERT INTO articles (url, status_code, content)
        VALUES ($1, $2, $3)
        ON CONFLICT (url) DO UPDATE SET 
            status_code = EXCLUDED.status_code,
            content = EXCLUDED.content,
            timestamp = CURRENT_TIMESTAMP
        "#,
        article.url,
        article.status_code,
        article.content
    )
    .execute(&mut *tx)
    .await
    .context("Firecrawl記事のデータベースへの挿入に失敗しました")?;

    let inserted = if result.rows_affected() > 0 { 1 } else { 0 };

    tx.commit()
        .await
        .context("トランザクションのコミットに失敗しました")?;

    Ok(DatabaseInsertResult::new(inserted, 1 - inserted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_article_from_file() {
        // BBCのモックファイルを読み込んでパース
        let result = read_article_from_file("mock/fc/bbc.json");
        assert!(result.is_ok(), "Firecrawl JSONファイルの読み込みに失敗");

        let article = result.unwrap();

        // 基本的なフィールドの検証
        assert!(!article.content.is_empty(), "contentが空です");
        assert!(!article.url.is_empty(), "URLが空です");

        println!("✅ Firecrawlデータの読み込みテスト成功");
        println!("URL: {}", article.url);
        println!("Contentサイズ: {} characters", article.content.len());
        println!("Status Code: {:?}", article.status_code);
    }

    #[test]
    fn test_read_non_existing_file() {
        // 存在しないファイルを読み込もうとするテスト
        let result = read_article_from_file("non_existent_file.json");
        assert!(result.is_err(), "存在しないファイルでエラーにならなかった");
    }

    // データベース保存機能のテスト

    // テスト例1: Firecrawl記事の基本的な保存機能のテスト
    #[sqlx::test]
    async fn test_save_article_to_db(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        // テスト用のFirecrawl記事データを作成
        let now = OffsetDateTime::now_utc();
        let test_article = Article {
            url: "https://test.example.com/firecrawl".to_string(),
            timestamp: now,
            status_code: Some(200),
            content: "# Test Article\n\nThis is a test content.".to_string(),
        };

        // データベースに保存をテスト
        let result = save_article_with_pool(&test_article, &pool).await?;

        // SaveResultの検証
        assert_eq!(result.inserted, 1, "新規挿入された記事数が期待と異なります");
        assert_eq!(
            result.skipped_duplicate, 0,
            "重複スキップ数が期待と異なります"
        );

        // 実際にデータベースに保存されたことを確認
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 1, "期待する件数(1件)が保存されませんでした");

        println!("✅ Firecrawl記事保存件数検証成功: {}件", result.inserted);
        println!(
            "✅ Firecrawl SaveResult検証成功: {}",
            result.display_with_domain("Firecrawlドキュメント")
        );

        Ok(())
    }

    // テスト例2: Firecrawl記事の重複処理テスト
    #[sqlx::test]
    async fn test_duplicate_articles(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let now = OffsetDateTime::now_utc();

        // 最初の記事を保存
        let original_article = Article {
            url: "https://test.example.com/duplicate".to_string(),
            timestamp: now,
            status_code: Some(200),
            content: "Original content".to_string(),
        };

        // 最初の記事を保存
        let result1 = save_article_with_pool(&original_article, &pool).await?;
        assert_eq!(result1.inserted, 1);

        // 同じURLで違う内容の記事を作成（重複）
        let duplicate_article = Article {
            url: "https://test.example.com/duplicate".to_string(),
            timestamp: now,
            status_code: Some(404),
            content: "Different content".to_string(),
        };

        // 重複記事を保存しようとする（新しい仕様では更新される）
        let result2 = save_article_with_pool(&duplicate_article, &pool).await?;

        // SaveResultの検証（更新される場合、inserted=1として扱う）
        assert_eq!(result2.inserted, 1, "重複URLの記事は更新されるべきです");
        assert_eq!(
            result2.skipped_duplicate, 0,
            "重複スキップ数が期待と異なります"
        );

        // データベースの件数は1件のまま
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 1, "重複記事が挿入され、件数が変わってしまいました");

        println!("✅ Firecrawl重複スキップ検証成功: {}", result2);

        Ok(())
    }
}
