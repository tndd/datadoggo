use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::service::{search_article_join_rows, ArticleJoinRowQuery};

// ユーザー側が実際に取り扱う情報モデル
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub url: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub content: String,
}

// ユーザーがArticleを取得する際に使用するクエリモデル
#[derive(Debug, Default)]
pub struct ArticleQuery {
    pub link_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}

// ユーザーがArticleを取得する際に使う関数（status_code=200のみ）
pub async fn search_articles(query: Option<ArticleQuery>, pool: &PgPool) -> Result<Vec<Article>> {
    let query = query.unwrap_or_default();

    // service層のクエリに変換
    let join_query = ArticleJoinRowQuery {
        link_pattern: query.link_pattern,
        pub_date_from: query.pub_date_from,
        pub_date_to: query.pub_date_to,
        status_codes: Some(vec![200]), // 成功したもののみ
        source: None,
        limit: query.limit,
    };

    let join_rows = search_article_join_rows(Some(join_query), pool).await?;

    // ArticleJoinRowからArticleに変換
    let articles: Result<Vec<Article>, _> = join_rows
        .into_iter()
        .filter_map(|row| {
            // status_code=200かつcontentが存在するもののみ
            if row.status_code == Some(200) && row.content.is_some() && row.timestamp.is_some() {
                Some(Ok(Article {
                    url: row.url,
                    title: row.title,
                    pub_date: row.pub_date,
                    updated_at: row.timestamp.unwrap(),
                    content: row.content.unwrap(),
                }))
            } else {
                None
            }
        })
        .collect();

    articles
}
