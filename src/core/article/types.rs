use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

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

// どのurlがどういうステータスを持っているかを確認するための軽量な構造体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleUrlStatus {
    pub url: String,
    pub status_code: Option<i32>,
}

// ArticleUrlStatusを取得する際に使用するクエリモデル
#[derive(Debug, Default)]
pub struct ArticleUrlStatusQuery {
    pub url_pattern: Option<String>,
    pub statuses: Option<Vec<ArticleStatus>>,
    pub limit: Option<i64>,
}

// ArticleLinkとArticleのJOIN結果をそのまま受け取るDB用の構造体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleJoinRow {
    pub url: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub source: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub status_code: Option<i32>,
    pub content: Option<String>,
}

// ArticleJoinRowを取得する際に使用するクエリモデル
#[derive(Debug, Default)]
pub struct ArticleJoinRowQuery {
    pub link_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
    pub statuses: Option<Vec<ArticleStatus>>,
    pub source: Option<String>,
    pub limit: Option<i64>,
}

// 記事内容の構造体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleContent {
    pub url: String,
    pub timestamp: DateTime<Utc>, // (updated_at)
    pub status_code: i32,
    pub content: String,
}

// ArticleContentを取得する際に使用するクエリモデル
#[derive(Debug, Default)]
pub struct ArticleContentQuery {
    pub url_pattern: Option<String>,
    pub timestamp_from: Option<DateTime<Utc>>,
    pub timestamp_to: Option<DateTime<Utc>>,
    pub status_code: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    mod article_status {
        use super::*;

        #[test]
        fn test_enum_variants() {
            let unprocessed = ArticleStatus::Unprocessed;
            let success = ArticleStatus::Success;
            let error_404 = ArticleStatus::Error(404);
            let error_500 = ArticleStatus::Error(500);

            // パターンマッチングの動作確認
            match unprocessed {
                ArticleStatus::Unprocessed => (),
                _ => panic!("Unprocessedのマッチングが失敗"),
            }

            match success {
                ArticleStatus::Success => (),
                _ => panic!("Successのマッチングが失敗"),
            }

            match error_404 {
                ArticleStatus::Error(404) => (),
                _ => panic!("Error(404)のマッチングが失敗"),
            }

            match error_500 {
                ArticleStatus::Error(code) => assert_eq!(code, 500),
                _ => panic!("Error(500)のマッチングが失敗"),
            }
        }

        #[test]
        fn test_serde_serialization() {
            let unprocessed = ArticleStatus::Unprocessed;
            let success = ArticleStatus::Success;
            let error = ArticleStatus::Error(404);

            // シリアライゼーション
            let unprocessed_json = serde_json::to_string(&unprocessed).unwrap();
            let success_json = serde_json::to_string(&success).unwrap();
            let error_json = serde_json::to_string(&error).unwrap();

            // デシリアライゼーション
            let deserialized_unprocessed: ArticleStatus =
                serde_json::from_str(&unprocessed_json).unwrap();
            let deserialized_success: ArticleStatus = serde_json::from_str(&success_json).unwrap();
            let deserialized_error: ArticleStatus = serde_json::from_str(&error_json).unwrap();

            // 検証
            assert!(matches!(
                deserialized_unprocessed,
                ArticleStatus::Unprocessed
            ));
            assert!(matches!(deserialized_success, ArticleStatus::Success));
            assert!(matches!(deserialized_error, ArticleStatus::Error(404)));
        }
    }

    mod article_url_status {
        use super::*;

        #[test]
        fn test_creation() {
            let status = ArticleUrlStatus {
                url: "https://example.com".to_string(),
                status_code: Some(200),
            };

            assert_eq!(status.url, "https://example.com");
            assert_eq!(status.status_code, Some(200));
        }

        #[test]
        fn test_with_null_status_code() {
            let status = ArticleUrlStatus {
                url: "https://unprocessed.com".to_string(),
                status_code: None,
            };

            assert_eq!(status.url, "https://unprocessed.com");
            assert_eq!(status.status_code, None);
        }

        #[test]
        fn test_serde_serialization() {
            let status = ArticleUrlStatus {
                url: "https://example.com".to_string(),
                status_code: Some(200),
            };

            let json = serde_json::to_string(&status).unwrap();
            let deserialized: ArticleUrlStatus = serde_json::from_str(&json).unwrap();

            assert_eq!(deserialized.url, status.url);
            assert_eq!(deserialized.status_code, status.status_code);
        }
    }

    mod query_models {
        use super::*;

        #[test]
        fn test_article_url_status_query_default() {
            let query = ArticleUrlStatusQuery::default();

            assert!(query.url_pattern.is_none());
            assert!(query.statuses.is_none());
            assert!(query.limit.is_none());
        }

        #[test]
        fn test_article_join_row_query_default() {
            let query = ArticleJoinRowQuery::default();

            assert!(query.link_pattern.is_none());
            assert!(query.pub_date_from.is_none());
            assert!(query.pub_date_to.is_none());
            assert!(query.statuses.is_none());
            assert!(query.source.is_none());
            assert!(query.limit.is_none());
        }

        #[test]
        fn test_article_content_query_default() {
            let query = ArticleContentQuery::default();

            assert!(query.url_pattern.is_none());
            assert!(query.timestamp_from.is_none());
            assert!(query.timestamp_to.is_none());
            assert!(query.status_code.is_none());
        }

        #[test]
        fn test_query_creation_with_values() {
            let now = Utc::now();
            let statuses = vec![ArticleStatus::Success, ArticleStatus::Error(404)];

            let url_query = ArticleUrlStatusQuery {
                url_pattern: Some("example.com".to_string()),
                statuses: Some(statuses.clone()),
                limit: Some(10),
            };

            let join_query = ArticleJoinRowQuery {
                link_pattern: Some("tech.example.com".to_string()),
                pub_date_from: Some(now),
                pub_date_to: Some(now),
                statuses: Some(statuses.clone()),
                source: Some("tech".to_string()),
                limit: Some(5),
            };

            let content_query = ArticleContentQuery {
                url_pattern: Some("news.example.com".to_string()),
                timestamp_from: Some(now),
                timestamp_to: Some(now),
                status_code: Some(200),
            };

            // 値が正しく設定されているか確認
            assert_eq!(url_query.url_pattern.unwrap(), "example.com");
            assert_eq!(url_query.limit.unwrap(), 10);
            assert_eq!(url_query.statuses.unwrap().len(), 2);

            assert_eq!(join_query.link_pattern.unwrap(), "tech.example.com");
            assert_eq!(join_query.source.unwrap(), "tech");
            assert_eq!(join_query.limit.unwrap(), 5);

            assert_eq!(content_query.url_pattern.unwrap(), "news.example.com");
            assert_eq!(content_query.status_code.unwrap(), 200);
        }
    }

    mod article_join_row {
        use super::*;

        #[test]
        fn test_creation_with_full_data() {
            let now = Utc::now();
            let pub_date = Utc.with_ymd_and_hms(2025, 1, 15, 10, 0, 0).unwrap();

            let row = ArticleJoinRow {
                url: "https://example.com/article".to_string(),
                title: "Test Article".to_string(),
                pub_date,
                source: "tech".to_string(),
                timestamp: Some(now),
                status_code: Some(200),
                content: Some("Article content".to_string()),
            };

            assert_eq!(row.url, "https://example.com/article");
            assert_eq!(row.title, "Test Article");
            assert_eq!(row.pub_date, pub_date);
            assert_eq!(row.source, "tech");
            assert_eq!(row.timestamp, Some(now));
            assert_eq!(row.status_code, Some(200));
            assert_eq!(row.content, Some("Article content".to_string()));
        }

        #[test]
        fn test_creation_with_null_optional_fields() {
            let pub_date = Utc.with_ymd_and_hms(2025, 1, 15, 10, 0, 0).unwrap();

            let row = ArticleJoinRow {
                url: "https://example.com/unprocessed".to_string(),
                title: "Unprocessed Article".to_string(),
                pub_date,
                source: "news".to_string(),
                timestamp: None,
                status_code: None,
                content: None,
            };

            assert_eq!(row.url, "https://example.com/unprocessed");
            assert_eq!(row.title, "Unprocessed Article");
            assert!(row.timestamp.is_none());
            assert!(row.status_code.is_none());
            assert!(row.content.is_none());
        }

        #[test]
        fn test_serde_serialization() {
            let pub_date = Utc.with_ymd_and_hms(2025, 1, 15, 10, 0, 0).unwrap();

            let row = ArticleJoinRow {
                url: "https://example.com/article".to_string(),
                title: "Test Article".to_string(),
                pub_date,
                source: "tech".to_string(),
                timestamp: Some(Utc::now()),
                status_code: Some(200),
                content: Some("Content".to_string()),
            };

            let json = serde_json::to_string(&row).unwrap();
            let deserialized: ArticleJoinRow = serde_json::from_str(&json).unwrap();

            assert_eq!(deserialized.url, row.url);
            assert_eq!(deserialized.title, row.title);
            assert_eq!(deserialized.pub_date, row.pub_date);
            assert_eq!(deserialized.source, row.source);
        }
    }

    mod article_content {
        use super::*;

        #[test]
        fn test_creation() {
            let now = Utc::now();

            let content = ArticleContent {
                url: "https://example.com/article".to_string(),
                timestamp: now,
                status_code: 200,
                content: "Article content goes here".to_string(),
            };

            assert_eq!(content.url, "https://example.com/article");
            assert_eq!(content.timestamp, now);
            assert_eq!(content.status_code, 200);
            assert_eq!(content.content, "Article content goes here");
        }

        #[test]
        fn test_with_error_status() {
            let now = Utc::now();

            let content = ArticleContent {
                url: "https://example.com/not-found".to_string(),
                timestamp: now,
                status_code: 404,
                content: "Page not found".to_string(),
            };

            assert_eq!(content.status_code, 404);
            assert_eq!(content.content, "Page not found");
        }

        #[test]
        fn test_with_empty_content() {
            let now = Utc::now();

            let content = ArticleContent {
                url: "https://example.com/empty".to_string(),
                timestamp: now,
                status_code: 200,
                content: String::new(),
            };

            assert_eq!(content.status_code, 200);
            assert!(content.content.is_empty());
        }

        #[test]
        fn test_with_large_content() {
            let now = Utc::now();
            let large_content = "A".repeat(10000); // 10KB

            let content = ArticleContent {
                url: "https://example.com/large".to_string(),
                timestamp: now,
                status_code: 200,
                content: large_content.clone(),
            };

            assert_eq!(content.content.len(), 10000);
            assert_eq!(content.content, large_content);
        }

        #[test]
        fn test_serde_serialization() {
            let now = Utc::now();

            let content = ArticleContent {
                url: "https://example.com/article".to_string(),
                timestamp: now,
                status_code: 200,
                content: "Content".to_string(),
            };

            let json = serde_json::to_string(&content).unwrap();
            let deserialized: ArticleContent = serde_json::from_str(&json).unwrap();

            assert_eq!(deserialized.url, content.url);
            assert_eq!(deserialized.timestamp, content.timestamp);
            assert_eq!(deserialized.status_code, content.status_code);
            assert_eq!(deserialized.content, content.content);
        }
    }
}
