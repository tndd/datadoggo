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

// 記事の処理状態を表現するenum（service.rsに移動）
// ArticleStatusはservice.rsで定義されているため、こちらでは削除

// ArticleMetadataの処理関数（ArticleStatusの参照を削除し、直接判定）

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

/// 記事ステータスでフィルタリングする関数（ArticleMetadata用）
pub fn filter_articles_metadata_by_status(
    articles: &[ArticleMetadata],
    status: crate::core::article::service::ArticleStatus,
) -> Vec<&ArticleMetadata> {
    articles
        .iter()
        .filter(|article| {
            let article_status = match article.status_code {
                None => crate::core::article::service::ArticleStatus::Unprocessed,
                Some(200) => crate::core::article::service::ArticleStatus::Success,
                Some(code) => crate::core::article::service::ArticleStatus::Error(code),
            };
            match status {
                crate::core::article::service::ArticleStatus::Unprocessed => {
                    matches!(
                        article_status,
                        crate::core::article::service::ArticleStatus::Unprocessed
                    )
                }
                crate::core::article::service::ArticleStatus::Success => {
                    matches!(
                        article_status,
                        crate::core::article::service::ArticleStatus::Success
                    )
                }
                crate::core::article::service::ArticleStatus::Error(code) => {
                    matches!(
                        article_status,
                        crate::core::article::service::ArticleStatus::Error(c) if c == code
                    )
                }
            }
        })
        .collect()
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

// 以下は削除された構造体への参照があった古い関数群
// 新しいドメインモデルに対応したものはmod.rsまたはservice.rsに移動

#[cfg(test)]
mod tests {
    use super::*;

    // ドメインロジック・振る舞い系テスト
    mod domain {
        use super::*;

        // ArticleMetadata関連のテスト
        #[test]
        fn test_article_metadata_functions() {
            let articles = vec![
                ArticleMetadata {
                    url: "https://test.com/unprocessed".to_string(),
                    title: "未処理記事".to_string(),
                    pub_date: chrono::Utc::now(),
                    updated_at: None,
                    status_code: None,
                },
                ArticleMetadata {
                    url: "https://test.com/success".to_string(),
                    title: "成功記事".to_string(),
                    pub_date: chrono::Utc::now(),
                    updated_at: Some(chrono::Utc::now()),
                    status_code: Some(200),
                },
                ArticleMetadata {
                    url: "https://test.com/error".to_string(),
                    title: "エラー記事".to_string(),
                    pub_date: chrono::Utc::now(),
                    updated_at: Some(chrono::Utc::now()),
                    status_code: Some(404),
                },
            ];

            // バックログフォーマットのテスト
            let backlog = format_backlog_articles_metadata(&articles);
            assert_eq!(backlog.len(), 2); // 未処理とエラー記事
            assert!(backlog[0].contains("未処理記事"));
            assert!(backlog[1].contains("エラー記事"));

            // 統計計算のテスト
            let (unprocessed, success, error) = count_articles_metadata_by_status(&articles);
            assert_eq!((unprocessed, success, error), (1, 1, 1));

            println!("✅ ArticleMetadata関数テスト成功");
        }

        // 直接フィールドアクセスのテスト
        #[test]
        fn test_direct_field_access() {
            let light_article = ArticleMetadata {
                url: "https://test.com/light".to_string(),
                title: "軽量版記事".to_string(),
                pub_date: chrono::Utc::now(),
                updated_at: Some(chrono::Utc::now()),
                status_code: Some(404),
            };

            // 直接フィールドアクセス
            assert_eq!(light_article.url, "https://test.com/light");
            assert_eq!(light_article.title, "軽量版記事");
            assert_eq!(light_article.status_code, Some(404));

            // ArticleMetadataでのバックログ判定
            let is_backlog =
                light_article.status_code.is_none() || light_article.status_code != Some(200);
            assert!(is_backlog);

            println!("✅ 直接フィールドアクセステスト成功");
        }
    }
}
