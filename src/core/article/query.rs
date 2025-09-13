use crate::core::article::model::ArticleStatus;
use chrono::{DateTime, Utc};

// ユーザーがArticleを取得する際に使用するクエリモデル（ドメイン向け）
#[derive(Debug, Default)]
pub struct ArticleQuery {
    pub link_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}

// ArticleUrlStatusを取得する際に使用するクエリモデル
#[derive(Debug, Default)]
pub struct ArticleUrlStatusQuery {
    pub url_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
    pub statuses: Option<Vec<ArticleStatus>>,
    pub limit: Option<i64>,
}

// ArticleJoinRowを取得する際に使用するクエリモデル
#[derive(Debug, Default)]
pub(crate) struct ArticleJoinRowQuery {
    pub link_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
    pub statuses: Option<Vec<ArticleStatus>>,
    pub source: Option<String>,
    pub limit: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    mod article_query {
        use super::*;

        /// ArticleQueryのデフォルト値テスト
        /// 目的: Default traitの正常な動作を確認
        #[test]
        fn test_article_query_default() {
            let query = ArticleQuery::default();
            assert!(query.link_pattern.is_none());
            assert!(query.pub_date_from.is_none());
            assert!(query.pub_date_to.is_none());
            assert!(query.limit.is_none());
        }

        /// ArticleQueryの部分的なフィールド設定テスト
        /// 目的: 一部のフィールドのみ設定した場合の動作確認
        #[test]
        fn test_article_query_partial_fields() {
            let query = ArticleQuery {
                link_pattern: Some("example.com".to_string()),
                limit: Some(10),
                ..Default::default()
            };
            assert_eq!(query.link_pattern, Some("example.com".to_string()));
            assert_eq!(query.limit, Some(10));
            assert!(query.pub_date_from.is_none());
            assert!(query.pub_date_to.is_none());
        }

        /// ArticleQueryの全フィールド設定テスト
        /// 目的: 全てのフィールドが正常に設定されることを確認
        #[test]
        fn test_article_query_all_fields() {
            let from_date = Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap();
            let to_date = Utc.with_ymd_and_hms(2023, 12, 31, 23, 59, 59).unwrap();

            let query = ArticleQuery {
                link_pattern: Some("test.com".to_string()),
                pub_date_from: Some(from_date),
                pub_date_to: Some(to_date),
                limit: Some(100),
            };

            assert_eq!(query.link_pattern, Some("test.com".to_string()));
            assert_eq!(query.pub_date_from, Some(from_date));
            assert_eq!(query.pub_date_to, Some(to_date));
            assert_eq!(query.limit, Some(100));
        }
    }

    mod article_url_status_query {
        use super::*;

        /// ArticleUrlStatusQueryのデフォルト値テスト
        /// 目的: Default traitの正常な動作を確認
        #[test]
        fn test_article_url_status_query_default() {
            let query = ArticleUrlStatusQuery::default();
            assert!(query.url_pattern.is_none());
            assert!(query.pub_date_from.is_none());
            assert!(query.pub_date_to.is_none());
            assert!(query.statuses.is_none());
            assert!(query.limit.is_none());
        }

        /// ArticleUrlStatusQueryのフィールド設定テスト
        /// 目的: 各フィールドが正常に設定されることを確認
        #[test]
        fn test_article_url_status_query_fields() {
            let from_date = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
            let to_date = Utc.with_ymd_and_hms(2025, 1, 1, 23, 59, 59).unwrap();
            let statuses = vec![ArticleStatus::Success, ArticleStatus::Error(404)];
            let query = ArticleUrlStatusQuery {
                url_pattern: Some("example.com".to_string()),
                pub_date_from: Some(from_date),
                pub_date_to: Some(to_date),
                statuses: Some(statuses.clone()),
                limit: Some(50),
            };

            assert_eq!(query.url_pattern, Some("example.com".to_string()));
            assert_eq!(query.pub_date_from, Some(from_date));
            assert_eq!(query.pub_date_to, Some(to_date));
            assert_eq!(query.statuses, Some(statuses));
            assert_eq!(query.limit, Some(50));
        }
    }

    mod article_join_row_query {
        use super::*;

        /// ArticleJoinRowQueryのデフォルト値テスト
        /// 目的: Default traitの正常な動作を確認
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

        /// ArticleJoinRowQueryの全フィールド設定テスト
        /// 目的: 全てのフィールドが正常に設定されることを確認
        #[test]
        fn test_article_join_row_query_all_fields() {
            let from_date = Utc.with_ymd_and_hms(2023, 6, 1, 0, 0, 0).unwrap();
            let to_date = Utc.with_ymd_and_hms(2023, 6, 30, 23, 59, 59).unwrap();
            let statuses = vec![ArticleStatus::Success];

            let query = ArticleJoinRowQuery {
                link_pattern: Some("news.com".to_string()),
                pub_date_from: Some(from_date),
                pub_date_to: Some(to_date),
                statuses: Some(statuses.clone()),
                source: Some("rss".to_string()),
                limit: Some(200),
            };

            assert_eq!(query.link_pattern, Some("news.com".to_string()));
            assert_eq!(query.pub_date_from, Some(from_date));
            assert_eq!(query.pub_date_to, Some(to_date));
            assert_eq!(query.statuses, Some(statuses));
            assert_eq!(query.source, Some("rss".to_string()));
            assert_eq!(query.limit, Some(200));
        }
    }
}
