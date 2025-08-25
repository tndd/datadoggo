use crate::rss_reader::RssArticle;
use sqlx::{Error as SqlxError, PgPool};
use std::env;

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

// 記事を一括挿入（パフォーマンスとエラーハンドリング両立）
pub async fn save_articles_to_db(articles: &[RssArticle]) -> Result<(), SqlxError> {
    let pool = create_pool().await?;

    if articles.is_empty() {
        println!("挿入する記事がありません");
        return Ok(());
    }

    const CHUNK_SIZE: usize = 1000; // 一括処理のチャンクサイズ
    let mut total_inserted = 0u64;
    let mut failed_articles = Vec::new();

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

        match query_builder.build().execute(&pool).await {
            Ok(result) => total_inserted += result.rows_affected(),
            Err(e) => {
                eprintln!("チャンク挿入エラー: {} - 個別処理に切り替えます", e);

                // チャンク全体の挿入が失敗した場合、個別に処理
                for article in chunk {
                    let result = sqlx::query(
                        "INSERT INTO rss_articles (title, link, description, pub_date) 
                         VALUES ($1, $2, $3, $4) 
                         ON CONFLICT (link) DO NOTHING",
                    )
                    .bind(&article.title)
                    .bind(&article.link)
                    .bind(article.description.as_deref().unwrap_or(""))
                    .bind(article.pub_date.as_deref().unwrap_or(""))
                    .execute(&pool)
                    .await;

                    match result {
                        Ok(result) => total_inserted += result.rows_affected(),
                        Err(e) => {
                            failed_articles.push((article.link.clone(), e.to_string()));
                        }
                    }
                }
            }
        }
    }

    // 結果レポート
    println!("新規挿入された記事数: {}", total_inserted);
    if !failed_articles.is_empty() {
        eprintln!("挿入失敗した記事数: {}", failed_articles.len());
        for (link, error) in failed_articles.iter().take(5) {
            eprintln!("  - {}: {}", link, error);
        }
        if failed_articles.len() > 5 {
            eprintln!("  ... 他{}件", failed_articles.len() - 5);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // テスト用のデータベースヘルパー
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
            let test_id = format!("test_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
            
            Ok(TestDb { pool, test_id })
        }

        // テスト用記事を挿入（プレフィックス付き）
        async fn insert_test_articles(&self, articles: &[RssArticle]) -> Result<Vec<RssArticle>, SqlxError> {
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
                     VALUES ($1, $2, $3, $4)"
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
                "SELECT COUNT(*) FROM rss_articles WHERE title LIKE $1 OR link LIKE $2"
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
                if let Err(e) = sqlx::query("DELETE FROM rss_articles WHERE title LIKE $1 OR link LIKE $2")
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

    // カスタムマクロ：テストセットアップとクリーンアップを自動化
    macro_rules! db_test {
        ($test_name:ident, $test_body:expr) => {
            #[tokio::test]
            async fn $test_name() {
                let test_db = match TestDb::new().await {
                    Ok(db) => db,
                    Err(e) => {
                        if e.to_string().contains("connection refused")
                            || e.to_string().contains("could not connect")
                        {
                            println!("⚠️  PostgreSQLが起動していません。'docker-compose -f docker-compose-db.yml up -d' でDBを起動してください");
                            return;
                        } else {
                            panic!("データベース接続エラー: {}", e);
                        }
                    }
                };

                // テストの結果を保存する変数
                let mut test_result = Ok(());

                // テスト本体を実行（panicをキャッチ）
                let result = tokio::task::spawn(async move {
                    $test_body(test_db).await
                }).await;

                match result {
                    Ok(Ok(())) => {
                        println!("✅ テスト成功: {}", stringify!($test_name));
                    }
                    Ok(Err(e)) => {
                        test_result = Err(format!("テストエラー: {:?}", e));
                    }
                    Err(e) => {
                        test_result = Err(format!("テスト実行エラー: {:?}", e));
                    }
                }

                // 結果をチェックしてpanicする場合
                if let Err(e) = test_result {
                    panic!("{}", e);
                }
            }
        };
    }

    // テスト例1: 基本的な保存機能のテスト
    db_test!(test_save_articles_to_db, |test_db: TestDb| async move {
        // テスト用記事データを作成
        let test_articles = vec![
            RssArticle {
                title: "Test Article 1".to_string(),
                link: "https://test.example.com/article1".to_string(),
                description: Some("Test description 1".to_string()),
                pub_date: Some("2025-08-24T00:00:00Z".to_string()),
            },
            RssArticle {
                title: "Test Article 2".to_string(),
                link: "https://test.example.com/article2".to_string(),
                description: Some("Test description 2".to_string()),
                pub_date: Some("2025-08-24T01:00:00Z".to_string()),
            },
        ];

        // データベースに保存をテスト
        save_articles_to_db(&test_articles).await?;
        
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    });

    // テスト例2: 重複記事の処理テスト
    db_test!(test_duplicate_articles, |test_db: TestDb| async move {
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
        save_articles_to_db(&initial_articles).await?;
        let final_count = test_db.count_test_articles().await?;
        
        // 重複なので挿入されない（countは変わらない）
        assert_eq!(final_count, 1);
        
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    });

    // テスト例3: 空の配列のテスト
    db_test!(test_empty_articles, |_test_db: TestDb| async move {
        let empty_articles: Vec<RssArticle> = vec![];
        save_articles_to_db(&empty_articles).await?;
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    });
}
