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
        let mut query_builder =
            sqlx::QueryBuilder::new("INSERT INTO articles (title, link, description, pub_date) ");

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

}
