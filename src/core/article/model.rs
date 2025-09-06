use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

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

// 軽量記事情報（処理判断用）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleInfo {
    pub url: String,
    pub status_code: Option<i32>, // None=未取得, Some(200)=成功, Some(4xx)=失敗
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

// ArticleInfoの処理状態判定メソッド
impl ArticleInfo {
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

/// バックログ記事をフォーマットする関数（ArticleInfo用）
pub fn format_backlog_article_info(articles: &[ArticleInfo]) -> Vec<String> {
    articles
        .iter()
        .filter(|article| article.is_backlog())
        .map(|article| {
            format!(
                "処理待ち: {} - {}",
                article.url,
                match article.status_code {
                    None => "未処理".to_string(),
                    Some(code) => format!("エラー({})", code),
                }
            )
        })
        .collect()
}

/// 記事ステータスでフィルタリングする関数（ArticleInfo用）
pub fn filter_article_info_by_status(
    articles: &[ArticleInfo],
    status: ArticleStatus,
) -> Vec<&ArticleInfo> {
    articles
        .iter()
        .filter(|article| {
            let article_status = article.get_article_status();
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

/// 記事統計情報を計算する関数（ArticleInfo用）
pub fn count_article_info_by_status(articles: &[ArticleInfo]) -> (usize, usize, usize) {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ドメインロジック・振る舞い系テスト
    mod domain {
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
        fn test_article_info_status_detection() {
            // 未処理記事のテスト
            let unprocessed = ArticleInfo {
                url: "https://test.com/unprocessed".to_string(),
                status_code: None,
            };
            assert!(matches!(
                unprocessed.get_article_status(),
                ArticleStatus::Unprocessed
            ));
            assert!(unprocessed.is_unprocessed());
            assert!(!unprocessed.is_error());
            assert!(unprocessed.is_backlog());

            // 成功記事のテスト
            let success = ArticleInfo {
                url: "https://test.com/success".to_string(),
                status_code: Some(200),
            };
            assert!(matches!(
                success.get_article_status(),
                ArticleStatus::Success
            ));
            assert!(!success.is_unprocessed());
            assert!(!success.is_error());
            assert!(!success.is_backlog());

            // エラー記事のテスト
            let error = ArticleInfo {
                url: "https://test.com/error".to_string(),
                status_code: Some(404),
            };
            assert!(matches!(
                error.get_article_status(),
                ArticleStatus::Error(404)
            ));
            assert!(!error.is_unprocessed());
            assert!(error.is_error());
            assert!(error.is_backlog());

            println!("✅ ArticleInfo状態判定テスト成功");
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

            // 軽量版記事のテスト
            let light_article = ArticleInfo {
                url: "https://test.com/light".to_string(),
                status_code: Some(404),
            };

            // 直接フィールドアクセス
            assert_eq!(full_article.url, "https://test.com/full");
            assert_eq!(full_article.title, "完全版記事");
            assert_eq!(full_article.status_code, 200);
            assert!(!full_article.is_error());

            assert_eq!(light_article.url, "https://test.com/light");
            assert_eq!(light_article.status_code, Some(404));
            assert!(light_article.is_backlog());

            println!("✅ 直接フィールドアクセステスト成功");
        }

        #[test]
        fn test_generic_functions() {
            let article_info = vec![
                ArticleInfo {
                    url: "https://test.com/unprocessed".to_string(),
                    status_code: None,
                },
                ArticleInfo {
                    url: "https://test.com/success".to_string(),
                    status_code: Some(200),
                },
                ArticleInfo {
                    url: "https://test.com/error".to_string(),
                    status_code: Some(404),
                },
            ];

            // 処理関数のテスト
            let backlog = format_backlog_article_info(&article_info);
            assert_eq!(backlog.len(), 2); // unprocessed + error
            assert!(backlog[0].contains("未処理"));
            assert!(backlog[1].contains("エラー(404)"));

            // ステータスフィルタリングのテスト
            let error_articles =
                filter_article_info_by_status(&article_info, ArticleStatus::Error(404));
            assert_eq!(error_articles.len(), 1);
            assert_eq!(error_articles[0].url, "https://test.com/error");

            let success_articles =
                filter_article_info_by_status(&article_info, ArticleStatus::Success);
            assert_eq!(success_articles.len(), 1);
            assert_eq!(success_articles[0].url, "https://test.com/success");

            // 統計計算のテスト
            let (unprocessed, success, error) = count_article_info_by_status(&article_info);
            assert_eq!((unprocessed, success, error), (1, 1, 1));

            println!("✅ 関数テスト成功");
        }
    }
}
