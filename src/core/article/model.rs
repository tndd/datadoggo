use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// 軽量記事エンティティ（バックログ処理用、contentを除外）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleMetadata {
    pub url: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub status_code: Option<i32>,
}

// 記事エンティティ（RSSリンクと記事内容の統合表現）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Article {
    pub url: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub status_code: Option<i32>,
    pub content: Option<String>,
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
            None => ArticleStatus::Unprocessed,
            Some(200) => ArticleStatus::Success,
            Some(code) => ArticleStatus::Error(code),
        }
    }
    /// 未処理のリンクかどうかを判定
    pub fn is_unprocessed(&self) -> bool {
        matches!(self.get_article_status(), ArticleStatus::Unprocessed)
    }
    /// エラー状態のリンクかどうかを判定
    pub fn is_error(&self) -> bool {
        matches!(self.get_article_status(), ArticleStatus::Error(_))
    }
    /// バックログ対象のリンクかどうかを判定
    pub fn is_backlog(&self) -> bool {
        self.is_unprocessed() || self.is_error()
    }
}

/// バックログ記事をフォーマットする関数（Article用）
pub fn format_backlog_articles(articles: &[Article]) -> Vec<String> {
    articles
        .iter()
        .filter(|article| article.is_backlog())
        .map(|article| format!("処理待ち: {} - {}", article.title, article.url))
        .collect()
}

/// バックログ記事をフォーマットする関数（ArticleMetadata用）
pub fn format_backlog_articles_metadata(articles: &[ArticleMetadata]) -> Vec<String> {
    articles
        .iter()
        .filter(|article| match article.status_code {
            None => true,
            Some(200) => false,
            Some(_) => true,
        })
        .map(|article| format!("処理待ち: {} - {}", article.title, article.url))
        .collect()
}

/// 記事ステータスでフィルタリングする関数（Article用）
pub fn filter_articles_by_status(articles: &[Article], status: ArticleStatus) -> Vec<&Article> {
    articles
        .iter()
        .filter(|article| match status {
            ArticleStatus::Unprocessed => article.is_unprocessed(),
            ArticleStatus::Success => {
                matches!(article.get_article_status(), ArticleStatus::Success)
            }
            ArticleStatus::Error(code) => {
                matches!(article.get_article_status(), ArticleStatus::Error(c) if c == code)
            }
        })
        .collect()
}

/// 記事ステータスでフィルタリングする関数（ArticleMetadata用）
pub fn filter_articles_metadata_by_status(
    articles: &[ArticleMetadata],
    status: ArticleStatus,
) -> Vec<&ArticleMetadata> {
    articles
        .iter()
        .filter(|article| {
            let article_status = match article.status_code {
                None => ArticleStatus::Unprocessed,
                Some(200) => ArticleStatus::Success,
                Some(code) => ArticleStatus::Error(code),
            };
            match status {
                ArticleStatus::Unprocessed => matches!(article_status, ArticleStatus::Unprocessed),
                ArticleStatus::Success => matches!(article_status, ArticleStatus::Success),
                ArticleStatus::Error(code) => {
                    matches!(article_status, ArticleStatus::Error(c) if c == code)
                }
            }
        })
        .collect()
}

/// 記事統計情報を計算する関数（Article用）
pub fn count_articles_by_status(articles: &[Article]) -> (usize, usize, usize) {
    let mut unprocessed = 0;
    let mut success = 0;
    let mut error = 0;

    for article in articles {
        match article.get_article_status() {
            ArticleStatus::Unprocessed => unprocessed += 1,
            ArticleStatus::Success => success += 1,
            ArticleStatus::Error(_) => error += 1,
        }
    }

    (unprocessed, success, error)
}

/// 記事統計情報を計算する関数（ArticleMetadata用）
pub fn count_articles_metadata_by_status(articles: &[ArticleMetadata]) -> (usize, usize, usize) {
    let mut unprocessed = 0;
    let mut success = 0;
    let mut error = 0;

    for article in articles {
        match article.status_code {
            None => unprocessed += 1,
            Some(200) => success += 1,
            Some(_) => error += 1,
        }
    }

    (unprocessed, success, error)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ドメインロジック・振る舞い系テスト
    mod domain {
        use super::*;

        // 記事ステータス判定機能

        #[test]
        fn test_article_status_detection() {
            // 未処理リンクのテスト
            let unprocessed = Article {
                url: "https://test.com/unprocessed".to_string(),
                title: "未処理記事".to_string(),
                pub_date: Utc::now(),
                updated_at: None,
                status_code: None,
                content: None,
            };
            assert!(matches!(
                unprocessed.get_article_status(),
                ArticleStatus::Unprocessed
            ));
            assert!(unprocessed.is_unprocessed());
            assert!(!unprocessed.is_error());
            assert!(unprocessed.is_backlog());
            // 成功記事のテスト
            let success = Article {
                url: "https://test.com/success".to_string(),
                title: "成功記事".to_string(),
                pub_date: Utc::now(),
                updated_at: Some(Utc::now()),
                status_code: Some(200),
                content: Some("記事内容".to_string()),
            };
            assert!(matches!(
                success.get_article_status(),
                ArticleStatus::Success
            ));
            assert!(!success.is_unprocessed());
            assert!(!success.is_error());
            assert!(!success.is_backlog());
            // エラー記事のテスト
            let error = Article {
                url: "https://test.com/error".to_string(),
                title: "エラー記事".to_string(),
                pub_date: Utc::now(),
                updated_at: Some(Utc::now()),
                status_code: Some(404),
                content: Some("エラー内容".to_string()),
            };
            assert!(matches!(
                error.get_article_status(),
                ArticleStatus::Error(404)
            ));
            assert!(!error.is_unprocessed());
            assert!(error.is_error());
            assert!(error.is_backlog());

            println!("✅ Article状態判定テスト成功");
        }

        // 直接フィールドアクセスのテスト
        #[test]
        fn test_direct_field_access() {
            // 完全版記事のテスト
            let full_article = Article {
                url: "https://test.com/full".to_string(),
                title: "完全版記事".to_string(),
                pub_date: Utc::now(),
                updated_at: Some(Utc::now()),
                status_code: Some(200),
                content: Some("記事内容".to_string()),
            };
            // 軽量版記事のテスト
            let light_article = ArticleMetadata {
                url: "https://test.com/light".to_string(),
                title: "軽量版記事".to_string(),
                pub_date: Utc::now(),
                updated_at: Some(Utc::now()),
                status_code: Some(404),
            };
            // 直接フィールドアクセス
            assert_eq!(full_article.url, "https://test.com/full");
            assert_eq!(full_article.title, "完全版記事");
            assert_eq!(full_article.status_code, Some(200));
            assert!(!full_article.is_backlog());

            assert_eq!(light_article.url, "https://test.com/light");
            assert_eq!(light_article.title, "軽量版記事");
            assert_eq!(light_article.status_code, Some(404));
            // ArticleMetadataにはis_backlogメソッドがないため、直接判定
            let is_backlog =
                light_article.status_code.is_none() || light_article.status_code != Some(200);
            assert!(is_backlog);

            println!("✅ 直接フィールドアクセステスト成功");
        }

        #[test]
        fn test_generic_functions() {
            let full_articles = vec![
                Article {
                    url: "https://test.com/success".to_string(),
                    title: "成功記事".to_string(),
                    pub_date: Utc::now(),
                    updated_at: Some(Utc::now()),
                    status_code: Some(200),
                    content: Some("成功内容".to_string()),
                },
                Article {
                    url: "https://test.com/error".to_string(),
                    title: "エラー記事".to_string(),
                    pub_date: Utc::now(),
                    updated_at: Some(Utc::now()),
                    status_code: Some(404),
                    content: Some("エラー内容".to_string()),
                },
            ];

            let light_articles = vec![
                ArticleMetadata {
                    url: "https://test.com/unprocessed".to_string(),
                    title: "未処理記事".to_string(),
                    pub_date: Utc::now(),
                    updated_at: None,
                    status_code: None,
                },
                ArticleMetadata {
                    url: "https://test.com/success_light".to_string(),
                    title: "成功軽量記事".to_string(),
                    pub_date: Utc::now(),
                    updated_at: Some(Utc::now()),
                    status_code: Some(200),
                },
            ];
            // 処理関数のテスト
            let full_backlog = format_backlog_articles(&full_articles);
            let light_backlog = format_backlog_articles_metadata(&light_articles);

            assert_eq!(full_backlog.len(), 1);
            assert!(full_backlog[0].contains("エラー記事"));
            assert_eq!(light_backlog.len(), 1);
            assert!(light_backlog[0].contains("未処理記事"));
            // ステータスフィルタリングのテスト
            let error_articles =
                filter_articles_by_status(&full_articles, ArticleStatus::Error(404));
            assert_eq!(error_articles.len(), 1);
            assert_eq!(error_articles[0].title, "エラー記事");

            let success_light =
                filter_articles_metadata_by_status(&light_articles, ArticleStatus::Success);
            assert_eq!(success_light.len(), 1);
            assert_eq!(success_light[0].title, "成功軽量記事");
            // 統計計算のテスト
            let (unprocessed, success, error) = count_articles_by_status(&full_articles);
            assert_eq!((unprocessed, success, error), (0, 1, 1));

            let (light_unprocessed, light_success, light_error) =
                count_articles_metadata_by_status(&light_articles);
            assert_eq!((light_unprocessed, light_success, light_error), (1, 1, 0));

            println!("✅ 関数テスト成功");
        }
    }
}
