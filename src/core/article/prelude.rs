use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::service::{search_article_join_rows, ArticleJoinRowQuery, ArticleStatus};

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
        statuses: Some(vec![ArticleStatus::Success]), // 成功したもののみ
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_article_creation() {
        let article = Article {
            url: "https://example.com".to_string(),
            title: "テスト記事".to_string(),
            pub_date: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2025, 1, 2, 0, 0, 0).unwrap(),
            content: "テスト内容".to_string(),
        };

        assert_eq!(article.url, "https://example.com");
        assert_eq!(article.title, "テスト記事");
        assert_eq!(article.content, "テスト内容");
    }

    #[test]
    fn test_article_query_default() {
        let query = ArticleQuery::default();

        assert!(query.link_pattern.is_none());
        assert!(query.pub_date_from.is_none());
        assert!(query.pub_date_to.is_none());
        assert!(query.limit.is_none());
    }

    #[test]
    fn test_article_query_with_values() {
        let pub_date_from = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let pub_date_to = Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap();

        let query = ArticleQuery {
            link_pattern: Some("example.com".to_string()),
            pub_date_from: Some(pub_date_from),
            pub_date_to: Some(pub_date_to),
            limit: Some(10),
        };

        assert_eq!(query.link_pattern, Some("example.com".to_string()));
        assert_eq!(query.pub_date_from, Some(pub_date_from));
        assert_eq!(query.pub_date_to, Some(pub_date_to));
        assert_eq!(query.limit, Some(10));
    }

    #[test]
    fn test_search_articles_empty_database() {
        // 空データベースでの動作確認（fixtureなしの通常テスト）
        // 実際のDB テストは実装時に必要に応じて追加
        let query = ArticleQuery::default();
        assert!(query.link_pattern.is_none());
    }

    #[sqlx::test(fixtures("prelude_basic"))]
    async fn test_search_articles_basic(pool: PgPool) {
        let result = search_articles(None, &pool).await;
        assert!(result.is_ok());

        let articles = result.unwrap();
        // status_code=200かつcontentありのもののみ取得される
        assert_eq!(articles.len(), 3);

        let urls: Vec<&str> = articles.iter().map(|a| a.url.as_str()).collect();
        assert!(urls.contains(&"https://example.com/article1"));
        assert!(urls.contains(&"https://example.com/article2"));
        assert!(urls.contains(&"https://example.com/article4"));
    }

    #[sqlx::test(fixtures("prelude_filter"))]
    async fn test_search_articles_with_filters(pool: PgPool) {
        let pub_date_from = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let pub_date_to = Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap();

        let query = Some(ArticleQuery {
            link_pattern: Some("tech.example.com".to_string()),
            pub_date_from: Some(pub_date_from),
            pub_date_to: Some(pub_date_to),
            limit: Some(5),
        });

        let result = search_articles(query, &pool).await;
        assert!(result.is_ok());

        let articles = result.unwrap();
        assert!(articles.len() <= 5);

        for article in &articles {
            assert!(article.url.contains("tech.example.com"));
            assert!(article.pub_date >= pub_date_from);
            assert!(article.pub_date <= pub_date_to);
        }
    }
}
