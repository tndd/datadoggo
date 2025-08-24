use crate::rss_reader::RssArticle;
use sqlx::{Error as SqlxError, PgPool};
use std::env;

// データベース接続プールを作成
async fn create_pool() -> Result<PgPool, SqlxError> {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://datadoggo:datadoggo@localhost:15432/datadoggo".to_string()
    });

    PgPool::connect(&database_url).await
}

// テーブルが存在することを確認
async fn ensure_table_exists(pool: &PgPool) -> Result<(), SqlxError> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS articles (
         title       TEXT NOT NULL,
         link        TEXT PRIMARY KEY,
         description TEXT,
         pub_date    TEXT
         )",
    )
    .execute(pool)
    .await?;

    Ok(())
}

// 記事を個別に挿入する版（エラーハンドリング重視）
pub async fn save_articles_to_db(articles: &[RssArticle]) -> Result<(), SqlxError> {
    let pool = create_pool().await?;
    ensure_table_exists(&pool).await?;

    let mut inserted_count = 0;
    for article in articles {
        let result = sqlx::query(
            "INSERT INTO articles (title, link, description, pub_date) 
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
            Ok(result) => inserted_count += result.rows_affected(),
            Err(e) => eprintln!("記事の挿入に失敗: {} - エラー: {}", article.link, e),
        }
    }

    println!("新規挿入された記事数: {}", inserted_count);
    Ok(())
}

// 一括挿入版（パフォーマンス重視）
pub async fn save_articles_to_db_batch(articles: &[RssArticle]) -> Result<(), SqlxError> {
    let pool = create_pool().await?;
    ensure_table_exists(&pool).await?;

    if articles.is_empty() {
        println!("挿入する記事がありません");
        return Ok(());
    }

    // 一括挿入でパフォーマンスを向上
    let mut query_builder =
        sqlx::QueryBuilder::new("INSERT INTO articles (title, link, description, pub_date) ");

    query_builder.push_values(articles, |mut b, article| {
        b.push_bind(&article.title)
            .push_bind(&article.link)
            .push_bind(article.description.as_deref().unwrap_or(""))
            .push_bind(article.pub_date.as_deref().unwrap_or(""));
    });

    query_builder.push(" ON CONFLICT (link) DO NOTHING");

    let result = query_builder.build().execute(&pool).await?;
    println!("新規挿入された記事数: {}", result.rows_affected());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_save_articles_to_db() {
        // 環境変数を読み込み
        let _ = dotenvy::dotenv();

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
        let result = save_articles_to_db(&test_articles).await;

        match result {
            Ok(_) => println!("✅ データベーステスト成功"),
            Err(e) => {
                // PostgreSQLが起動していない場合はスキップ
                if e.to_string().contains("connection refused")
                    || e.to_string().contains("could not connect")
                {
                    println!("⚠️  PostgreSQLが起動していません。'docker-compose -f docker-compose-db.yml up -d' でDBを起動してください");
                } else {
                    panic!("データベーステストでエラー: {}", e);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_save_articles_to_db_batch() {
        // 環境変数を読み込み
        let _ = dotenvy::dotenv();

        // テスト用記事データを作成
        let test_articles = vec![
            RssArticle {
                title: "Batch Test Article 1".to_string(),
                link: "https://batch.example.com/article1".to_string(),
                description: Some("Batch test description 1".to_string()),
                pub_date: Some("2025-08-24T02:00:00Z".to_string()),
            },
            RssArticle {
                title: "Batch Test Article 2".to_string(),
                link: "https://batch.example.com/article2".to_string(),
                description: Some("Batch test description 2".to_string()),
                pub_date: Some("2025-08-24T03:00:00Z".to_string()),
            },
        ];

        // 一括挿入をテスト
        let result = save_articles_to_db_batch(&test_articles).await;

        match result {
            Ok(_) => println!("✅ 一括挿入テスト成功"),
            Err(e) => {
                // PostgreSQLが起動していない場合はスキップ
                if e.to_string().contains("connection refused")
                    || e.to_string().contains("could not connect")
                {
                    println!("⚠️  PostgreSQLが起動していません。'docker-compose -f docker-compose-db.yml up -d' でDBを起動してください");
                } else {
                    panic!("一括挿入テストでエラー: {}", e);
                }
            }
        }
    }
}
