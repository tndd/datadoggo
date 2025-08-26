use crate::rss_reader::RssArticle;
use sqlx::{Error as SqlxError, PgPool};
use std::env;
use std::fmt;

/// データベースへの保存結果を格納する構造体
#[derive(Debug, Clone)]
pub struct SaveResult {
    /// 新規にデータベースに挿入された記事数
    pub inserted: u64,
    /// 重複によりスキップされた記事数
    pub skipped: usize,
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

// データベース接続プールを作成
async fn create_pool() -> Result<PgPool, SqlxError> {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://datadoggo:datadoggo@localhost:15432/datadoggo".to_string()
    });
    let pool = PgPool::connect(&database_url).await?;
    // マイグレーションを実行
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

/// # 概要
/// RssArticleの配列をデータベースに保存する。
///
/// ## 動作
/// - 1000件ずつのチャンクに分けて一括INSERT
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
pub async fn save_articles_to_db(articles: &[RssArticle]) -> Result<SaveResult, SqlxError> {
    let pool = create_pool().await?;

    if articles.is_empty() {
        return Ok(SaveResult {
            inserted: 0,
            skipped: 0,
        });
    }

    // トランザクションを開始
    let mut tx = pool.begin().await?;

    const CHUNK_SIZE: usize = 1000; // 一括処理のチャンクサイズ
    let mut total_inserted = 0u64;

    // チャンクごとに一括処理
    for chunk in articles.chunks(CHUNK_SIZE) {
        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO rss_articles (title, link, description, pub_date) ",
        );

        query_builder.push_values(chunk, |mut b, article| {
            b.push_bind(&article.title)
                .push_bind(&article.link)
                .push_bind(article.description.as_deref().unwrap_or(""))
                .push_bind(article.pub_date.as_deref().unwrap_or(""));
        });

        query_builder.push(" ON CONFLICT (link) DO NOTHING");

        let result = query_builder.build().execute(&mut *tx).await?;
        total_inserted += result.rows_affected();
    }

    // トランザクションをコミット
    tx.commit().await?;

    // 結果を構造体として返す
    let total_processed = articles.len();
    let skipped = total_processed - total_inserted as usize;

    Ok(SaveResult {
        inserted: total_inserted,
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// # 概要
    /// テスト用のデータベースを扱うための構造体
    ///
    /// # 主要機能
    /// - テスト用の接続先を作成する
    /// - テスト後のクリーンアップを行う
    struct TestDb {
        pool: PgPool,
        test_id: String,
    }

    impl TestDb {
        // テスト用データベース接続を作成
        async fn new() -> Result<Self, SqlxError> {
            let _ = dotenvy::dotenv();

            let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgresql://datadoggo:datadoggo@localhost:15432/datadoggo".to_string()
            });

            let pool = PgPool::connect(&database_url).await?;
            sqlx::migrate!("./migrations").run(&pool).await?;

            // テストごとにユニークな識別子を生成
            let test_id = format!(
                "test_{}",
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
            );

            Ok(TestDb { pool, test_id })
        }

        // テスト用記事を挿入（プレフィックス付き）
        async fn insert_test_articles(
            &self,
            articles: &[RssArticle],
        ) -> Result<Vec<RssArticle>, SqlxError> {
            let mut test_articles = Vec::new();

            for article in articles {
                let test_article = RssArticle {
                    title: format!("[{}] {}", self.test_id, article.title),
                    link: format!("{}?test_id={}", article.link, self.test_id),
                    description: article.description.clone(),
                    pub_date: article.pub_date.clone(),
                };

                sqlx::query(
                    "INSERT INTO rss_articles (title, link, description, pub_date) 
                     VALUES ($1, $2, $3, $4)",
                )
                .bind(&test_article.title)
                .bind(&test_article.link)
                .bind(test_article.description.as_deref().unwrap_or(""))
                .bind(test_article.pub_date.as_deref().unwrap_or(""))
                .execute(&self.pool)
                .await?;

                test_articles.push(test_article);
            }

            Ok(test_articles)
        }

        // 記事数を取得（テスト用データのみ）
        async fn count_test_articles(&self) -> Result<i64, SqlxError> {
            let row = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM rss_articles WHERE title LIKE $1 OR link LIKE $2",
            )
            .bind(format!("[{}]%", self.test_id))
            .bind(format!("%test_id={}", self.test_id))
            .fetch_one(&self.pool)
            .await?;
            Ok(row)
        }
    }

    impl Drop for TestDb {
        fn drop(&mut self) {
            // クリーンアップを非同期で実行（ベストエフォート）
            let pool = self.pool.clone();
            let test_id = self.test_id.clone();

            tokio::spawn(async move {
                if let Err(e) =
                    sqlx::query("DELETE FROM rss_articles WHERE title LIKE $1 OR link LIKE $2")
                        .bind(format!("[{}]%", test_id))
                        .bind(format!("%test_id={}", test_id))
                        .execute(&pool)
                        .await
                {
                    eprintln!("テストデータクリーンアップエラー: {}", e);
                }
            });
        }
    }

    // テスト用のエラー型
    type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

    // テスト用のヘルパー関数
    async fn setup_test_db() -> TestResult<TestDb> {
        match TestDb::new().await {
            Ok(db) => Ok(db),
            Err(e) => {
                if e.to_string().contains("connection refused")
                    || e.to_string().contains("could not connect")
                {
                    println!("⚠️  PostgreSQLが起動していません。'docker-compose -f docker-compose-db.yml up -d' でDBを起動してください");
                    // テストをスキップするためのカスタムエラー
                    return Err("Database connection failed - PostgreSQL not running".into());
                } else {
                    return Err(e.into());
                }
            }
        }
    }

    // テスト例1: 基本的な保存機能のテスト
    #[tokio::test]
    async fn test_save_articles_to_db() -> TestResult<()> {
        let test_db = match setup_test_db().await {
            Ok(db) => db,
            Err(_) => {
                // データベース接続に失敗した場合はテストをスキップ
                return Ok(());
            }
        };

        // テスト前の件数を確認
        let count_before = test_db.count_test_articles().await?;
        assert_eq!(
            count_before, 0,
            "テスト開始前にテストデータが存在しています"
        );

        // テスト用記事データを作成
        let test_articles = vec![
            RssArticle {
                title: format!("[{}] Test Article 1", test_db.test_id),
                link: format!(
                    "https://test.example.com/article1?test_id={}",
                    test_db.test_id
                ),
                description: Some("Test description 1".to_string()),
                pub_date: Some("2025-08-24T00:00:00Z".to_string()),
            },
            RssArticle {
                title: format!("[{}] Test Article 2", test_db.test_id),
                link: format!(
                    "https://test.example.com/article2?test_id={}",
                    test_db.test_id
                ),
                description: Some("Test description 2".to_string()),
                pub_date: Some("2025-08-24T01:00:00Z".to_string()),
            },
        ];

        // データベースに保存をテスト
        let result = save_articles_to_db(&test_articles).await?;

        // SaveResultの検証
        assert_eq!(result.inserted, 2, "新規挿入された記事数が期待と異なります");
        assert_eq!(result.skipped, 0, "重複スキップ数が期待と異なります");

        // 保存後の件数を確認
        let count_after = test_db.count_test_articles().await?;
        assert_eq!(
            count_after, 2,
            "期待する件数(2件)が保存されませんでした。実際の件数: {}",
            count_after
        );

        println!("✅ 保存件数検証成功: {}件", result.inserted);
        println!("✅ SaveResult検証成功: {}", result);
        Ok(())
    }

    // テスト例2: 重複記事の処理テスト
    #[tokio::test]
    async fn test_duplicate_articles() -> TestResult<()> {
        let test_db = match setup_test_db().await {
            Ok(db) => db,
            Err(_) => {
                // データベース接続に失敗した場合はテストをスキップ
                return Ok(());
            }
        };

        let article = RssArticle {
            title: "Duplicate Test Article".to_string(),
            link: "https://test.example.com/duplicate".to_string(),
            description: Some("Test duplicate".to_string()),
            pub_date: Some("2025-08-24T02:00:00Z".to_string()),
        };

        // 初期データを挿入
        let initial_articles = test_db.insert_test_articles(&[article.clone()]).await?;
        let initial_count = test_db.count_test_articles().await?;
        assert_eq!(initial_count, 1);

        // 同じ記事を再度挿入（重複）
        let result = save_articles_to_db(&initial_articles).await?;
        let final_count = test_db.count_test_articles().await?;

        // SaveResultの検証
        assert_eq!(
            result.inserted, 0,
            "重複記事が新規挿入されるべきではありません"
        );
        assert_eq!(result.skipped, 1, "重複スキップ数が期待と異なります");

        // 重複なので挿入されない（countは変わらない）
        assert_eq!(final_count, 1);

        println!("✅ 重複スキップ検証成功: {}", result);

        Ok(())
    }

    // テスト例3: 空の配列のテスト
    #[tokio::test]
    async fn test_empty_articles() -> TestResult<()> {
        let _test_db = match setup_test_db().await {
            Ok(db) => db,
            Err(_) => {
                // データベース接続に失敗した場合はテストをスキップ
                return Ok(());
            }
        };

        let empty_articles: Vec<RssArticle> = vec![];
        let result = save_articles_to_db(&empty_articles).await?;

        // 空配列の結果検証
        assert_eq!(result.inserted, 0, "空配列の新規挿入数は0であるべきです");
        assert_eq!(result.skipped, 0, "空配列の重複スキップ数は0であるべきです");

        println!("✅ 空配列処理検証成功: {}", result);
        Ok(())
    }
}
