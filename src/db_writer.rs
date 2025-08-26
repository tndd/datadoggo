use crate::rss_reader::RssArticle;
use sqlx::{Error as SqlxError, PgPool, QueryBuilder};
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
async fn create_pool() -> Result<PgPool, SqlxError> {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://datadoggo:datadoggo@localhost:15432/datadoggo".to_string()
    });
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

/// RssArticleの配列を指定されたデータベースプールに保存する。\
/// 既にプールを準備している場合は `save_rss_articles_to_db` ではなく、この関数を使用する。
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
    // PostgreSQL制限を考慮した定数
    const MAX_BIND_PARAMS: usize = 65535; // PostgreSQL maximum
    const FIELDS_PER_ROW: usize = 4; // title, link, description, pub_date
    const SAFE_CHUNK_SIZE: usize = MAX_BIND_PARAMS / FIELDS_PER_ROW;
    // RSS記事テーブル用の定数
    const RSS_TABLE: &str = "rss_articles";
    const RSS_COLUMNS: &str = "title, link, description, pub_date";
    const RSS_CONFLICT: &str = "ON CONFLICT (link) DO NOTHING";

    let mut tx = pool.begin().await?;
    let mut total_inserted = 0;

    for chunk in articles.chunks(SAFE_CHUNK_SIZE.min(1000)) {
        let mut query_builder =
            QueryBuilder::new(format!("INSERT INTO {} ({})", RSS_TABLE, RSS_COLUMNS));

        query_builder.push_values(chunk, |mut b, article| {
            b.push_bind(&article.title)
                .push_bind(&article.link)
                .push_bind(article.description.as_deref().unwrap_or(""))
                .push_bind(article.pub_date.as_deref().unwrap_or(""));
        });

        query_builder.push(" ").push(RSS_CONFLICT);

        let result = query_builder.build().execute(&mut *tx).await?;
        total_inserted += result.rows_affected() as usize;
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

    // テストモジュール初期化時にDATABASE_URLを設定
    #[ctor::ctor]
    fn init_test_env() {
        env::set_var(
            "DATABASE_URL",
            "postgresql://datadoggo:datadoggo@localhost:15432/datadoggo",
        );
    }

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
}
