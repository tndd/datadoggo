use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// 記事のリンク情報を格納する構造体（<item>要素のみ対象）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleLink {
    pub url: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub source: String,
}

// 記事のフィルター条件を表す構造体
#[derive(Debug, Default)]
pub struct ArticleLinkQuery {
    pub link_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    mod article_link {
        use super::*;

        /// ArticleLink構造体のシリアライゼーションテスト
        /// 目的: JSONへの正常な変換を確認
        #[test]
        fn test_article_link_serialization() {
            let article_link = ArticleLink {
                url: "https://example.com/article".to_string(),
                title: "Test Article".to_string(),
                pub_date: Utc.with_ymd_and_hms(2023, 12, 25, 10, 30, 0).unwrap(),
                source: "rss".to_string(),
            };

            let json = serde_json::to_string(&article_link).unwrap();
            assert!(json.contains("\"url\":\"https://example.com/article\""));
            assert!(json.contains("\"title\":\"Test Article\""));
            assert!(json.contains("\"source\":\"rss\""));
        }

        /// ArticleLink構造体のデシリアライゼーションテスト
        /// 目的: JSONからの正常な復元を確認
        #[test]
        fn test_article_link_deserialization() {
            let json = r#"
            {
                "url": "https://example.com/article",
                "title": "Test Article",
                "pub_date": "2023-12-25T10:30:00Z",
                "source": "rss"
            }
            "#;

            let article_link: ArticleLink = serde_json::from_str(json).unwrap();
            assert_eq!(article_link.url, "https://example.com/article");
            assert_eq!(article_link.title, "Test Article");
            assert_eq!(article_link.source, "rss");
            assert_eq!(
                article_link.pub_date,
                Utc.with_ymd_and_hms(2023, 12, 25, 10, 30, 0).unwrap()
            );
        }

        /// ArticleLink構造体のクローンテスト
        /// 目的: Clone traitの正常動作を確認
        #[test]
        fn test_article_link_clone() {
            let original = ArticleLink {
                url: "https://example.com/article".to_string(),
                title: "Test Article".to_string(),
                pub_date: Utc.with_ymd_and_hms(2023, 12, 25, 10, 30, 0).unwrap(),
                source: "rss".to_string(),
            };

            let cloned = original.clone();
            assert_eq!(original.url, cloned.url);
            assert_eq!(original.title, cloned.title);
            assert_eq!(original.pub_date, cloned.pub_date);
            assert_eq!(original.source, cloned.source);
        }
    }

    mod article_link_query {
        use super::*;

        /// ArticleLinkQueryのデフォルト値テスト
        /// 目的: Default traitの正常な動作を確認
        #[test]
        fn test_article_link_query_default() {
            let query = ArticleLinkQuery::default();
            assert!(query.link_pattern.is_none());
            assert!(query.pub_date_from.is_none());
            assert!(query.pub_date_to.is_none());
        }

        /// ArticleLinkQueryの部分的なフィールド設定テスト
        /// 目的: 一部のフィールドのみ設定した場合の動作確認
        #[test]
        fn test_article_link_query_partial_fields() {
            let query = ArticleLinkQuery {
                link_pattern: Some("example.com".to_string()),
                ..Default::default()
            };
            assert_eq!(query.link_pattern, Some("example.com".to_string()));
            assert!(query.pub_date_from.is_none());
            assert!(query.pub_date_to.is_none());
        }

        /// ArticleLinkQueryの全フィールド設定テスト
        /// 目的: 全てのフィールドが正常に設定されることを確認
        #[test]
        fn test_article_link_query_all_fields() {
            let from_date = Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap();
            let to_date = Utc.with_ymd_and_hms(2023, 12, 31, 23, 59, 59).unwrap();

            let query = ArticleLinkQuery {
                link_pattern: Some("test.com".to_string()),
                pub_date_from: Some(from_date),
                pub_date_to: Some(to_date),
            };

            assert_eq!(query.link_pattern, Some("test.com".to_string()));
            assert_eq!(query.pub_date_from, Some(from_date));
            assert_eq!(query.pub_date_to, Some(to_date));
        }
    }
}
