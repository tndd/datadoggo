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
    let mut dropped_count = 0;
    let articles: Result<Vec<Article>, _> = join_rows
        .into_iter()
        .filter_map(|row| {
            // status_code=200かつcontentが存在するもののみ
            if row.status_code == Some(200) && row.content.is_some() && row.timestamp.is_some() {
                Some(Ok(Article {
                    url: row.url,
                    title: row.title,
                    pub_date: row.pub_date,
                    updated_at: row.timestamp.expect("フィルタ条件で確認済みのtimestampがNone"),
                    content: row.content.expect("フィルタ条件で確認済みのcontentがNone"),
                }))
            } else {
                // 無効なレコードを記録（デバッグ時に有用）
                dropped_count += 1;
                if cfg!(debug_assertions) {
                    eprintln!(
                        "記事レコードをスキップ: url={}, status_code={:?}, content_exists={}, timestamp_exists={}",
                        row.url,
                        row.status_code,
                        row.content.is_some(),
                        row.timestamp.is_some()
                    );
                }
                None
            }
        })
        .collect();

    if dropped_count > 0 {
        println!("{}件の無効な記事レコードをスキップしました", dropped_count);
    }

    articles
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone, Utc};

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
    fn test_article_query() {
        // デフォルト値のテスト
        let default_query = ArticleQuery::default();
        assert!(default_query.link_pattern.is_none());
        assert!(default_query.pub_date_from.is_none());
        assert!(default_query.pub_date_to.is_none());
        assert!(default_query.limit.is_none());

        // 値設定のテスト
        let pub_date_from = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let pub_date_to = Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap();

        let query_with_values = ArticleQuery {
            link_pattern: Some("example.com".to_string()),
            pub_date_from: Some(pub_date_from),
            pub_date_to: Some(pub_date_to),
            limit: Some(10),
        };

        assert_eq!(
            query_with_values.link_pattern,
            Some("example.com".to_string())
        );
        assert_eq!(query_with_values.pub_date_from, Some(pub_date_from));
        assert_eq!(query_with_values.pub_date_to, Some(pub_date_to));
        assert_eq!(query_with_values.limit, Some(10));
    }

    #[sqlx::test(fixtures("prelude_basic"))]
    async fn test_search_articles_basic(pool: PgPool) {
        let result = search_articles(None, &pool).await;
        assert!(result.is_ok());

        let articles = result.unwrap();
        // status_code=200かつcontentありのもののみ取得される（エッジケース含む）
        assert_eq!(articles.len(), 6);

        let urls: Vec<&str> = articles.iter().map(|a| a.url.as_str()).collect();
        assert!(urls.contains(&"https://example.com/article1"));
        assert!(urls.contains(&"https://example.com/article2"));
        assert!(urls.contains(&"https://example.com/article4"));
        assert!(urls.contains(&"https://example.com/special-chars"));
        assert!(urls.contains(&"https://example.com/empty-title"));
        assert!(urls.contains(&"https://very-long-domain-name-for-testing-url-limits.example.com/very/long/path/to/article"));

        // 特殊文字を含む記事の内容検証
        let special_chars_article = articles
            .iter()
            .find(|a| a.url == "https://example.com/special-chars")
            .expect("特殊文字記事が見つからない");
        assert_eq!(
            special_chars_article.title,
            "Title with \"quotes\" & <tags>"
        );
        assert!(special_chars_article.content.contains("éñüñ"));

        // 空タイトル記事の検証
        let empty_title_article = articles
            .iter()
            .find(|a| a.url == "https://example.com/empty-title")
            .expect("空タイトル記事が見つからない");
        assert_eq!(empty_title_article.title, "");
        assert_eq!(empty_title_article.content, "");

        // 長いURL/タイトル記事の検証
        let long_article = articles
            .iter()
            .find(|a| a.url.contains("very-long-domain"))
            .expect("長いURL記事が見つからない");
        assert!(long_article.title.len() > 50);
        assert!(long_article.content.len() > 100);

        // データの整合性確認: pub_dateとupdated_atの関係
        for article in &articles {
            assert!(
                article.updated_at >= article.pub_date,
                "updated_atはpub_date以降である必要がある"
            );
        }
    }

    #[sqlx::test(fixtures("prelude_filter"))]
    async fn test_search_articles_with_filters(pool: PgPool) {
        let pub_date_from = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let pub_date_to = Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap();

        // 基本的なフィルタリングテスト
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
        assert!(articles.len() >= 1); // 少なくとも1件は存在する

        for article in &articles {
            assert!(article.url.contains("tech.example.com"));
            assert!(article.pub_date >= pub_date_from);
            assert!(article.pub_date <= pub_date_to);
        }

        // 境界値テスト: 日付範囲境界での検証
        let exact_boundary_from = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let exact_boundary_to = Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap();

        let boundary_query = Some(ArticleQuery {
            link_pattern: Some("tech.example.com".to_string()),
            pub_date_from: Some(exact_boundary_from),
            pub_date_to: Some(exact_boundary_to),
            limit: None,
        });

        let boundary_result = search_articles(boundary_query, &pool).await;
        assert!(boundary_result.is_ok());
        let boundary_articles = boundary_result.unwrap();

        // 境界値に含まれる記事の確認
        let has_year_start = boundary_articles
            .iter()
            .any(|a| a.pub_date.year() == 2025 && a.pub_date.month() == 1 && a.pub_date.day() == 1);
        let has_year_end = boundary_articles.iter().any(|a| {
            a.pub_date.year() == 2025 && a.pub_date.month() == 12 && a.pub_date.day() == 31
        });

        assert!(has_year_start, "年始境界値の記事が含まれていない");
        assert!(has_year_end, "年末境界値の記事が含まれていない");

        // limitテスト: 0, 1, 大きな値での動作
        let limit_tests = [
            (Some(0i64), "limit=0で結果なし"),
            (Some(1i64), "limit=1で1件のみ"),
            (Some(100i64), "limit=100で全件取得可能"),
        ];

        for (limit_val, description) in limit_tests {
            let limit_query = Some(ArticleQuery {
                link_pattern: Some("tech.example.com".to_string()),
                pub_date_from: None,
                pub_date_to: None,
                limit: limit_val,
            });

            let limit_result = search_articles(limit_query, &pool).await;
            assert!(limit_result.is_ok(), "{}でエラーが発生", description);
            let limit_articles = limit_result.unwrap();

            match limit_val {
                Some(0) => assert_eq!(limit_articles.len(), 0, "limit=0で結果が0件でない"),
                Some(1) => assert!(limit_articles.len() <= 1, "limit=1で1件超過"),
                Some(100) => assert!(limit_articles.len() <= 100, "limit=100を超過"),
                _ => {}
            }
        }

        // パターンマッチング精度テスト
        let pattern_tests = [
            ("tech.example.com", true, "完全一致パターン"),
            ("tech", true, "部分一致パターン"),
            ("nonexistent.com", false, "存在しないドメインパターン"),
        ];

        for (pattern, should_have_results, description) in pattern_tests {
            let pattern_query = Some(ArticleQuery {
                link_pattern: Some(pattern.to_string()),
                pub_date_from: None,
                pub_date_to: None,
                limit: None,
            });

            let pattern_result = search_articles(pattern_query, &pool).await;
            assert!(pattern_result.is_ok(), "{}でエラーが発生", description);
            let pattern_articles = pattern_result.unwrap();

            if should_have_results {
                assert!(!pattern_articles.is_empty(), "{}で結果が0件", description);
                for article in &pattern_articles {
                    assert!(
                        article.url.contains(pattern),
                        "{}でパターン不一致: {}",
                        description,
                        article.url
                    );
                }
            } else {
                assert!(
                    pattern_articles.is_empty(),
                    "{}で予期しない結果",
                    description
                );
            }
        }
    }
}
