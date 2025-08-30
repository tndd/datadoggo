use crate::infra::db::setup_database;
use crate::infra::db::DatabaseInsertResult;
use crate::infra::loader::load_file;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

// Firecrawl記事構造体（テーブル定義と一致）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Article {
    pub url: String,
    pub timestamp: DateTime<Utc>,
    pub status_code: i32,
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

    // status_codeを取得（必須）
    let status_code = metadata
        .get("statusCode")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32)
        .ok_or_else(|| anyhow::anyhow!("statusCodeフィールドが見つかりません"))?;

    let now = Utc::now();

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

// Article記事のフィルター条件を表す構造体
#[derive(Debug, Default)]
pub struct ArticleFilter {
    pub url_pattern: Option<String>,
    pub timestamp_from: Option<DateTime<Utc>>,
    pub timestamp_to: Option<DateTime<Utc>>,
    pub status_code: Option<i32>,
}

/// # 概要
/// データベースからArticle記事を取得する。
///
/// ## 動作
/// - 自動でデータベース接続プールを作成
/// - 指定された条件でArticle記事を取得
///
/// ## 引数
/// - `filter`: フィルター条件。Noneの場合は全件取得
///
/// ## 戻り値
/// - `Vec<Article>`: 条件にマッチしたArticle記事のリスト
pub async fn search_articles_from_db(filter: Option<ArticleFilter>) -> Result<Vec<Article>> {
    let pool = setup_database().await?;
    search_articles_with_pool(filter, &pool).await
}

/// 指定されたデータベースプールからArticleを取得する。
pub async fn search_articles_with_pool(
    filter: Option<ArticleFilter>,
    pool: &PgPool,
) -> Result<Vec<Article>> {
    let filter = filter.unwrap_or_default();

    // QueryBuilderベースで動的にクエリを構築
    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT url, timestamp, status_code, content FROM articles",
    );

    let has_cond = filter.url_pattern.is_some()
        || filter.timestamp_from.is_some()
        || filter.timestamp_to.is_some()
        || filter.status_code.is_some();

    if has_cond {
        qb.push(" WHERE ");
        let mut separated = qb.separated(" AND ");

        if let Some(ref url_pattern) = filter.url_pattern {
            let url_query = format!("%{}%", url_pattern);
            separated.push("url ILIKE ").push_bind(url_query);
        }
        if let Some(ts_from) = filter.timestamp_from {
            separated.push("timestamp >= ").push_bind(ts_from);
        }
        if let Some(ts_to) = filter.timestamp_to {
            separated.push("timestamp <= ").push_bind(ts_to);
        }
        if let Some(status) = filter.status_code {
            separated.push("status_code = ").push_bind(status);
        }
    }

    qb.push(" ORDER BY timestamp DESC");

    let articles = qb.build_query_as::<Article>().fetch_all(pool).await?;

    Ok(articles)
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

    #[test]
    fn test_read_article_missing_status_code() {
        // statusCodeが存在しないJSONのテスト
        use std::fs;

        let json_content = r#"
        {
            "markdown": "テスト記事の内容です",
            "metadata": {
                "url": "https://test.example.com/article"
            }
        }
        "#;

        // 一時ファイル作成
        let temp_file = "temp_test_missing_status_code.json";
        fs::write(temp_file, json_content).expect("テストファイルの作成に失敗");

        // statusCodeが存在しない場合にエラーが返されることを確認
        let result = read_article_from_file(temp_file);
        assert!(
            result.is_err(),
            "statusCodeが存在しないのにエラーにならなかった"
        );

        // エラーメッセージの確認
        let error_message = result.unwrap_err().to_string();
        assert!(
            error_message.contains("statusCodeフィールドが見つかりません"),
            "期待されるエラーメッセージが含まれていません: {}",
            error_message
        );

        // テストファイル削除
        fs::remove_file(temp_file).ok();

        println!("✅ statusCode欠損エラーハンドリング検証成功");
    }

    // データベース保存機能のテスト

    // テスト例1: Firecrawl記事の基本的な保存機能のテスト
    #[sqlx::test]
    async fn test_save_article_to_db(pool: PgPool) -> Result<(), anyhow::Error> {
        // テスト用のFirecrawl記事データを作成
        let now = Utc::now();
        let test_article = Article {
            url: "https://test.example.com/firecrawl".to_string(),
            timestamp: now,
            status_code: 200,
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
        let count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, Some(1), "期待する件数(1件)が保存されませんでした");

        println!("✅ Firecrawl記事保存件数検証成功: {}件", result.inserted);
        println!(
            "✅ Firecrawl SaveResult検証成功: {}",
            result.display_with_domain("Firecrawlドキュメント")
        );

        Ok(())
    }

    // テスト例2: Firecrawl記事の重複処理テスト
    #[sqlx::test]
    async fn test_duplicate_articles(pool: PgPool) -> Result<(), anyhow::Error> {
        let now = Utc::now();

        // 最初の記事を保存
        let original_article = Article {
            url: "https://test.example.com/duplicate".to_string(),
            timestamp: now,
            status_code: 200,
            content: "Original content".to_string(),
        };

        // 最初の記事を保存
        let result1 = save_article_with_pool(&original_article, &pool).await?;
        assert_eq!(result1.inserted, 1);

        // 同じURLで違う内容の記事を作成（重複）
        let duplicate_article = Article {
            url: "https://test.example.com/duplicate".to_string(),
            timestamp: now,
            status_code: 404,
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
        let count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            count,
            Some(1),
            "重複記事が挿入され、件数が変わってしまいました"
        );

        println!("✅ Firecrawl重複スキップ検証成功: {}", result2);

        Ok(())
    }
}
