use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// 記事検索用クエリ（ユーザー向けドメインオブジェクト）
#[derive(Debug, Default)]
pub struct ArticleQuery {
    pub link_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}

// 記事エンティティ（完全な記事情報、非Optional）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Article {
    pub url: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status_code: i32,
    pub content: String,
}

// 記事の処理状態を表現するenum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArticleStatus {
    /// 記事が未処理（articleテーブルに存在しない）
    Unprocessed,
    /// 記事が正常に取得済み（status_code = 200）
    Success,
    /// 記事の取得にエラーが発生（status_code != 200）
    Error(i32),
}

// 記事の処理状態を判定するメソッド
impl Article {
    /// 記事の処理状態を取得
    pub fn get_article_status(&self) -> ArticleStatus {
        match self.status_code {
            200 => ArticleStatus::Success,
            code => ArticleStatus::Error(code),
        }
    }

    /// エラー状態のリンクかどうかを判定
    pub fn is_error(&self) -> bool {
        matches!(self.get_article_status(), ArticleStatus::Error(_))
    }
}

/// 完全な記事データを取得する（処理済みの記事のみ）
pub async fn search_articles(
    query: Option<ArticleQuery>,
    pool: &sqlx::PgPool,
) -> anyhow::Result<Vec<Article>> {
    use anyhow::Context;
    use sqlx::QueryBuilder;

    let query = query.unwrap_or_default();

    let mut qb = QueryBuilder::<sqlx::Postgres>::new(
        r#"
        SELECT 
            al.url,
            al.title,
            al.pub_date,
            a.timestamp as updated_at,
            a.status_code,
            a.content
        FROM article_links al
        INNER JOIN articles a ON al.url = a.url
        "#,
    );

    let mut has_where = false;
    if let Some(ref link_pattern) = query.link_pattern {
        if !has_where {
            qb.push(" WHERE ");
            has_where = true;
        }
        let pattern = format!("%{}%", link_pattern);
        qb.push("al.url ILIKE ").push_bind(pattern);
    }
    if let Some(pub_date_from) = query.pub_date_from {
        if has_where {
            qb.push(" AND ");
        } else {
            qb.push(" WHERE ");
            has_where = true;
        }
        qb.push("al.pub_date >= ").push_bind(pub_date_from);
    }
    if let Some(pub_date_to) = query.pub_date_to {
        if has_where {
            qb.push(" AND ");
        } else {
            qb.push(" WHERE ");
            has_where = true;
        }
        qb.push("al.pub_date <= ").push_bind(pub_date_to);
    }

    qb.push(" ORDER BY al.pub_date DESC");
    if let Some(limit) = query.limit {
        qb.push(" LIMIT ").push_bind(limit);
    }

    let results = qb
        .build_query_as::<Article>()
        .fetch_all(pool)
        .await
        .context("記事情報の取得に失敗")?;

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_article_status_detection() {
        // 成功記事のテスト
        let success = Article {
            url: "https://test.com/success".to_string(),
            title: "成功記事".to_string(),
            pub_date: Utc::now(),
            updated_at: Utc::now(),
            status_code: 200,
            content: "記事内容".to_string(),
        };
        assert!(matches!(
            success.get_article_status(),
            ArticleStatus::Success
        ));
        assert!(!success.is_error());

        // エラー記事のテスト
        let error = Article {
            url: "https://test.com/error".to_string(),
            title: "エラー記事".to_string(),
            pub_date: Utc::now(),
            updated_at: Utc::now(),
            status_code: 404,
            content: "エラー内容".to_string(),
        };
        assert!(matches!(
            error.get_article_status(),
            ArticleStatus::Error(404)
        ));
        assert!(error.is_error());

        println!("✅ Article状態判定テスト成功");
    }

    #[test]
    fn test_direct_field_access() {
        // 完全版記事のテスト
        let full_article = Article {
            url: "https://test.com/full".to_string(),
            title: "完全版記事".to_string(),
            pub_date: Utc::now(),
            updated_at: Utc::now(),
            status_code: 200,
            content: "記事内容".to_string(),
        };

        // 直接フィールドアクセス
        assert_eq!(full_article.url, "https://test.com/full");
        assert_eq!(full_article.title, "完全版記事");
        assert_eq!(full_article.status_code, 200);
        assert!(!full_article.is_error());

        println!("✅ 直接フィールドアクセステスト成功");
    }
}
