use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ユーザー側が実際に取り扱う情報モデル（ドメイン向け）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub url: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub content: String,
}

// 記事の処理状態を表現するenum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    // 記事の公開日時。再取得優先度の判断などに用いる（低レベル用途）
    pub pub_date: DateTime<Utc>,
}

// ArticleLinkとArticleのJOIN結果をそのまま受け取るDB用の構造体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct ArticleJoinRow {
    pub url: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub source: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub status_code: Option<i32>,
    pub content: Option<String>,
}

// 記事内容の構造体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleContent {
    pub url: String,
    pub timestamp: DateTime<Utc>, // (updated_at)
    pub status_code: i32,
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    mod article {
        use super::*;

        /// Article構造体のシリアライゼーションテスト
        /// 目的: JSONへの正常な変換を確認
        #[test]
        fn test_article_serialization() {
            let article = Article {
                url: "https://example.com/article".to_string(),
                title: "Test Article".to_string(),
                pub_date: Utc.with_ymd_and_hms(2023, 12, 25, 10, 30, 0).unwrap(),
                updated_at: Utc.with_ymd_and_hms(2023, 12, 26, 15, 45, 0).unwrap(),
                content: "This is test content".to_string(),
            };

            let json = serde_json::to_string(&article).unwrap();
            assert!(json.contains("\"url\":\"https://example.com/article\""));
            assert!(json.contains("\"title\":\"Test Article\""));
            assert!(json.contains("\"content\":\"This is test content\""));
        }

        /// Article構造体のデシリアライゼーションテスト
        /// 目的: JSONからの正常な復元を確認
        #[test]
        fn test_article_deserialization() {
            let json = r#"
            {
                "url": "https://example.com/article",
                "title": "Test Article",
                "pub_date": "2023-12-25T10:30:00Z",
                "updated_at": "2023-12-26T15:45:00Z",
                "content": "This is test content"
            }
            "#;

            let article: Article = serde_json::from_str(json).unwrap();
            assert_eq!(article.url, "https://example.com/article");
            assert_eq!(article.title, "Test Article");
            assert_eq!(article.content, "This is test content");
            assert_eq!(
                article.pub_date,
                Utc.with_ymd_and_hms(2023, 12, 25, 10, 30, 0).unwrap()
            );
        }

        /// Article構造体のクローンテスト
        /// 目的: Clone traitの正常動作を確認
        #[test]
        fn test_article_clone() {
            let original = Article {
                url: "https://example.com/article".to_string(),
                title: "Test Article".to_string(),
                pub_date: Utc.with_ymd_and_hms(2023, 12, 25, 10, 30, 0).unwrap(),
                updated_at: Utc.with_ymd_and_hms(2023, 12, 26, 15, 45, 0).unwrap(),
                content: "This is test content".to_string(),
            };

            let cloned = original.clone();
            assert_eq!(original.url, cloned.url);
            assert_eq!(original.title, cloned.title);
            assert_eq!(original.pub_date, cloned.pub_date);
            assert_eq!(original.updated_at, cloned.updated_at);
            assert_eq!(original.content, cloned.content);
        }
    }

    mod article_status {
        use super::*;

        /// ArticleStatusのUnprocessedバリアントテスト
        /// 目的: Unprocessedバリアントの正常動作を確認
        #[test]
        fn test_article_status_unprocessed() {
            let status = ArticleStatus::Unprocessed;
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, "\"Unprocessed\"");

            let deserialized: ArticleStatus = serde_json::from_str(&json).unwrap();
            matches!(deserialized, ArticleStatus::Unprocessed);
        }

        /// ArticleStatusのSuccessバリアントテスト
        /// 目的: Successバリアントの正常動作を確認
        #[test]
        fn test_article_status_success() {
            let status = ArticleStatus::Success;
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, "\"Success\"");

            let deserialized: ArticleStatus = serde_json::from_str(&json).unwrap();
            matches!(deserialized, ArticleStatus::Success);
        }

        /// ArticleStatusのErrorバリアントテスト
        /// 目的: Errorバリアント（エラーコード付き）の正常動作を確認
        #[test]
        fn test_article_status_error() {
            let status = ArticleStatus::Error(404);
            let json = serde_json::to_string(&status).unwrap();
            assert!(json.contains("404"));

            let deserialized: ArticleStatus = serde_json::from_str(&json).unwrap();
            match deserialized {
                ArticleStatus::Error(code) => assert_eq!(code, 404),
                _ => panic!("Expected Error variant"),
            }
        }

        /// ArticleStatusのクローンテスト
        /// 目的: Clone traitの正常動作を確認（特にErrorバリアント）
        #[test]
        fn test_article_status_clone() {
            let original = ArticleStatus::Error(500);
            let cloned = original.clone();

            match (original, cloned) {
                (ArticleStatus::Error(code1), ArticleStatus::Error(code2)) => {
                    assert_eq!(code1, code2);
                }
                _ => panic!("Clone failed for Error variant"),
            }
        }
    }

    mod article_url_status {
        use super::*;

        /// ArticleUrlStatusの基本構造テスト
        /// 目的: 構造体の基本的な作成・フィールドアクセスを確認
        #[test]
        fn test_article_url_status_creation() {
            let url_status = ArticleUrlStatus {
                url: "https://example.com".to_string(),
                status_code: Some(200),
                pub_date: Utc.with_ymd_and_hms(2023, 12, 25, 10, 30, 0).unwrap(),
            };

            assert_eq!(url_status.url, "https://example.com");
            assert_eq!(url_status.status_code, Some(200));
            assert_eq!(
                url_status.pub_date,
                Utc.with_ymd_and_hms(2023, 12, 25, 10, 30, 0).unwrap()
            );
        }

        /// ArticleUrlStatusのシリアライゼーションテスト
        /// 目的: JSONへの正常な変換を確認
        #[test]
        fn test_article_url_status_serialization() {
            let url_status = ArticleUrlStatus {
                url: "https://example.com".to_string(),
                status_code: Some(404),
                pub_date: Utc.with_ymd_and_hms(2023, 12, 31, 23, 59, 59).unwrap(),
            };

            let json = serde_json::to_string(&url_status).unwrap();
            assert!(json.contains("\"url\":\"https://example.com\""));
            assert!(json.contains("\"status_code\":404"));
            assert!(json.contains("\"pub_date\":\"2023-12-31T23:59:59Z\""));
        }

        /// ArticleUrlStatusのNoneステータスコードテスト
        /// 目的: status_codeがNoneの場合の動作確認
        #[test]
        fn test_article_url_status_none_status() {
            let url_status = ArticleUrlStatus {
                url: "https://example.com".to_string(),
                status_code: None,
                pub_date: Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap(),
            };

            let json = serde_json::to_string(&url_status).unwrap();
            assert!(json.contains("\"url\":\"https://example.com\""));
            assert!(json.contains("\"status_code\":null"));
            assert!(json.contains("\"pub_date\":\"2023-01-01T00:00:00Z\""));
        }
    }

    mod article_join_row {
        use super::*;

        /// ArticleJoinRowの基本構造テスト
        /// 目的: 構造体の基本的な作成・フィールドアクセスを確認
        #[test]
        fn test_article_join_row_creation() {
            let join_row = ArticleJoinRow {
                url: "https://example.com".to_string(),
                title: "Test Article".to_string(),
                pub_date: Utc.with_ymd_and_hms(2023, 12, 25, 10, 30, 0).unwrap(),
                source: "RSS".to_string(),
                timestamp: Some(Utc.with_ymd_and_hms(2023, 12, 26, 10, 30, 0).unwrap()),
                status_code: Some(200),
                content: Some("Article content".to_string()),
            };

            assert_eq!(join_row.url, "https://example.com");
            assert_eq!(join_row.title, "Test Article");
            assert_eq!(join_row.source, "RSS");
            assert_eq!(join_row.status_code, Some(200));
            assert!(join_row.timestamp.is_some());
            assert!(join_row.content.is_some());
        }

        /// ArticleJoinRowのシリアライゼーションテスト
        /// 目的: JSONへの正常な変換を確認
        #[test]
        fn test_article_join_row_serialization() {
            let join_row = ArticleJoinRow {
                url: "https://example.com".to_string(),
                title: "Test Article".to_string(),
                pub_date: Utc.with_ymd_and_hms(2023, 12, 25, 10, 30, 0).unwrap(),
                source: "RSS".to_string(),
                timestamp: None,
                status_code: None,
                content: None,
            };

            let json = serde_json::to_string(&join_row).unwrap();
            assert!(json.contains("\"url\":\"https://example.com\""));
            assert!(json.contains("\"title\":\"Test Article\""));
            assert!(json.contains("\"source\":\"RSS\""));
            assert!(json.contains("\"timestamp\":null"));
            assert!(json.contains("\"status_code\":null"));
            assert!(json.contains("\"content\":null"));
        }
    }

    mod article_content {
        use super::*;

        /// ArticleContentの基本構造テスト
        /// 目的: 構造体の基本的な作成・フィールドアクセスを確認
        #[test]
        fn test_article_content_creation() {
            let content = ArticleContent {
                url: "https://example.com".to_string(),
                timestamp: Utc.with_ymd_and_hms(2023, 12, 25, 10, 30, 0).unwrap(),
                status_code: 200,
                content: "Article content here".to_string(),
            };

            assert_eq!(content.url, "https://example.com");
            assert_eq!(content.status_code, 200);
            assert_eq!(content.content, "Article content here");
            assert_eq!(
                content.timestamp,
                Utc.with_ymd_and_hms(2023, 12, 25, 10, 30, 0).unwrap()
            );
        }

        /// ArticleContentのシリアライゼーションテスト
        /// 目的: JSONへの正常な変換を確認
        #[test]
        fn test_article_content_serialization() {
            let content = ArticleContent {
                url: "https://example.com".to_string(),
                timestamp: Utc.with_ymd_and_hms(2023, 12, 25, 10, 30, 0).unwrap(),
                status_code: 404,
                content: "Not Found".to_string(),
            };

            let json = serde_json::to_string(&content).unwrap();
            assert!(json.contains("\"url\":\"https://example.com\""));
            assert!(json.contains("\"status_code\":404"));
            assert!(json.contains("\"content\":\"Not Found\""));
        }

        /// ArticleContentのクローンテスト
        /// 目的: Clone traitの正常動作を確認
        #[test]
        fn test_article_content_clone() {
            let original = ArticleContent {
                url: "https://example.com".to_string(),
                timestamp: Utc.with_ymd_and_hms(2023, 12, 25, 10, 30, 0).unwrap(),
                status_code: 200,
                content: "Article content".to_string(),
            };

            let cloned = original.clone();
            assert_eq!(original.url, cloned.url);
            assert_eq!(original.timestamp, cloned.timestamp);
            assert_eq!(original.status_code, cloned.status_code);
            assert_eq!(original.content, cloned.content);
        }
    }
}
