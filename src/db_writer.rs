use crate::rss_reader::RssArticle;
use std::env;
use tokio_postgres::{types::ToSql, Error, NoTls};

// 構造体のフィールドを自動的にINSERTするマクロ
macro_rules! insert_article {
    ($client:expr, $article:expr) => {
        {
            let params: &[&(dyn ToSql + Sync)] = &[
                &$article.title,
                &$article.link,
                &$article.description.as_deref().unwrap_or(""),
                &$article.pub_date.as_deref().unwrap_or("")
            ];
            $client.execute(
                "INSERT INTO articles (title, link, description, pub_date) VALUES ($1, $2, $3, $4) ON CONFLICT (link) DO NOTHING",
                params
            ).await
        }
    };
}

// データベース接続を確立するヘルパー関数
async fn establish_connection() -> Result<tokio_postgres::Client, Error> {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "host=localhost port=15432 user=datadoggo password=datadoggo dbname=datadoggo".to_string()
    });

    let (client, connection) = tokio_postgres::connect(&database_url, NoTls).await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("データベース接続エラー: {}", e);
        }
    });

    Ok(client)
}

// テーブル作成のヘルパー関数
async fn ensure_table_exists(client: &tokio_postgres::Client) -> Result<(), Error> {
    client
        .execute(
            "CREATE TABLE IF NOT EXISTS articles (
         title       TEXT NOT NULL,
         link        TEXT PRIMARY KEY,
         description TEXT,
         pub_date    TEXT
     )",
            &[],
        )
        .await?;
    Ok(())
}

pub async fn save_articles_to_db(articles: &[RssArticle]) -> Result<(), Error> {
    let client = establish_connection().await?;
    ensure_table_exists(&client).await?;

    let mut inserted_count = 0;
    for article in articles {
        match insert_article!(client, article) {
            Ok(affected_rows) => inserted_count += affected_rows,
            Err(e) => eprintln!("記事の挿入に失敗しました: {} - エラー: {}", article.link, e),
        }
    }

    println!("新規挿入された記事数: {}", inserted_count);
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
                if e.to_string().contains("connection refused") {
                    println!("⚠️  PostgreSQLが起動していません。'docker-compose -f docker-compose-db.yml up -d' でDBを起動してください");
                } else {
                    panic!("データベーステストでエラー: {}", e);
                }
            }
        }
    }
}
