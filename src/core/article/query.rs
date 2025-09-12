use anyhow::{Context, Result};
use sqlx::PgPool;

use super::model::{
    Article, ArticleContent, ArticleContentQuery, ArticleJoinRow, ArticleJoinRowQuery,
    ArticleQuery, ArticleStatus, ArticleUrlStatus, ArticleUrlStatusQuery,
};

use super::service::normalize_statuses;

/// ArticleUrlStatusを取得する（読み取り）
pub async fn search_article_url_statuses(
    query: Option<ArticleUrlStatusQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleUrlStatus>> {
    let query = query.unwrap_or_default();
    let (apply_status, has_unprocessed, has_success, error_codes) =
        normalize_statuses(query.statuses.as_deref());

    let sql = r#"
        SELECT l.url, a.status_code
        FROM article_links AS l
        LEFT JOIN articles AS a ON l.url = a.url
        WHERE ($1::text IS NULL OR l.url ILIKE '%' || $1 || '%')
          AND (
                NOT $2
             OR (
                    (COALESCE($3::bool, false) AND a.status_code IS NULL)
                 OR (COALESCE($4::bool, false) AND a.status_code = 200)
                 OR ($5::int[] IS NOT NULL AND a.status_code = ANY($5))
                )
          )
        ORDER BY l.url ASC
        LIMIT COALESCE($6, NULL)
    "#;

    let results = sqlx::query_as::<_, ArticleUrlStatus>(sql)
        .bind(query.url_pattern)
        .bind(apply_status)
        .bind(has_unprocessed)
        .bind(has_success)
        .bind(error_codes)
        .bind(query.limit.map(|v| v as i64))
        .fetch_all(pool)
        .await
        .context("記事URL状態情報の取得に失敗")?;

    Ok(results)
}

/// ArticleJoinRowを取得する（読み取り）
pub async fn search_article_join_rows(
    query: Option<ArticleJoinRowQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleJoinRow>> {
    let query = query.unwrap_or_default();
    let (apply_status, has_unprocessed, has_success, error_codes) =
        normalize_statuses(query.statuses.as_deref());

    let sql = r#"
        SELECT l.url, l.title, l.pub_date, l.source,
               a.timestamp, a.status_code, a.content
        FROM article_links AS l
        LEFT JOIN articles AS a ON l.url = a.url
        WHERE ($1::text IS NULL OR l.url ILIKE '%' || $1 || '%')
          AND ($2::timestamptz IS NULL OR l.pub_date >= $2)
          AND ($3::timestamptz IS NULL OR l.pub_date <= $3)
          AND (
                NOT $4
             OR (
                    (COALESCE($5::bool, false) AND a.status_code IS NULL)
                 OR (COALESCE($6::bool, false) AND a.status_code = 200)
                 OR ($7::int[] IS NOT NULL AND a.status_code = ANY($7))
                )
          )
          AND ($8::text IS NULL OR l.source = $8)
        ORDER BY l.pub_date DESC
        LIMIT COALESCE($9, NULL)
    "#;

    let results = sqlx::query_as::<_, ArticleJoinRow>(sql)
        .bind(query.link_pattern)
        .bind(query.pub_date_from)
        .bind(query.pub_date_to)
        .bind(apply_status)
        .bind(has_unprocessed)
        .bind(has_success)
        .bind(error_codes)
        .bind(query.source)
        .bind(query.limit.map(|v| v as i64))
        .fetch_all(pool)
        .await
        .context("記事結合情報の取得に失敗")?;

    Ok(results)
}

/// ArticleContentを取得する（読み取り）
pub async fn search_article_contents(
    query: Option<ArticleContentQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleContent>> {
    let query = query.unwrap_or_default();

    let sql = r#"
        SELECT url, timestamp, status_code, content
        FROM articles
        WHERE ($1::text IS NULL OR url ILIKE '%' || $1 || '%')
          AND ($2::timestamptz IS NULL OR timestamp >= $2)
          AND ($3::timestamptz IS NULL OR timestamp <= $3)
          AND ($4::int IS NULL OR status_code = $4)
        ORDER BY timestamp DESC
    "#;

    let articles = sqlx::query_as::<_, ArticleContent>(sql)
        .bind(query.url_pattern)
        .bind(query.timestamp_from)
        .bind(query.timestamp_to)
        .bind(query.status_code)
        .fetch_all(pool)
        .await?;

    Ok(articles)
}

/// ドメイン向けArticleを取得（status_code=200のみ、結合行から変換）
pub async fn search_articles(query: Option<ArticleQuery>, pool: &PgPool) -> Result<Vec<Article>> {
    let query = query.unwrap_or_default();

    // 低レベルのJOIN検索クエリに変換
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
            if row.status_code == Some(200) && row.content.is_some() && row.timestamp.is_some() {
                Some(Ok(Article {
                    url: row.url,
                    title: row.title,
                    pub_date: row.pub_date,
                    updated_at: row.timestamp.expect("フィルタ条件で確認済みのtimestampがNone"),
                    content: row.content.expect("フィルタ条件で確認済みのcontentがNone"),
                }))
            } else {
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
    use chrono::{TimeZone, Utc};
    use sqlx::PgPool;

    // 関数名モジュール: search_article_url_statuses
    mod search_article_url_statuses {
        use super::*;
        use crate::core::article::model::ArticleStatus;

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_basic_search(pool: PgPool) -> Result<()> {
            let results = super::super::search_article_url_statuses(None, &pool).await?;
            assert!(results.len() >= 3);
            for i in 1..results.len() {
                assert!(results[i - 1].url <= results[i].url);
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_status_filtering(pool: PgPool) -> Result<()> {
            let success_query = ArticleUrlStatusQuery {
                statuses: Some(vec![ArticleStatus::Success]),
                ..Default::default()
            };
            let success_results =
                super::super::search_article_url_statuses(Some(success_query), &pool).await?;
            for result in &success_results {
                assert_eq!(result.status_code, Some(200));
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_search_patterns"))]
        async fn test_url_pattern_filtering(pool: PgPool) -> Result<()> {
            let domain_query = ArticleUrlStatusQuery {
                url_pattern: Some("example.com".to_string()),
                ..Default::default()
            };
            let domain_results =
                super::super::search_article_url_statuses(Some(domain_query), &pool).await?;
            for result in &domain_results {
                assert!(result.url.contains("example.com"));
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_limit_tests"))]
        async fn test_limit_filtering(pool: PgPool) -> Result<()> {
            let limit_query = ArticleUrlStatusQuery {
                limit: Some(2),
                ..Default::default()
            };
            let limit_results =
                super::super::search_article_url_statuses(Some(limit_query), &pool).await?;
            assert!(limit_results.len() <= 2);
            Ok(())
        }
    }

    // 関数名モジュール: search_article_join_rows
    mod search_article_join_rows {
        use super::*;
        use crate::core::article::model::ArticleStatus;

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_basic_search(pool: PgPool) -> Result<()> {
            let result = super::super::search_article_join_rows(None, &pool).await?;
            assert!(result.len() >= 1);
            for i in 1..result.len() {
                assert!(result[i - 1].pub_date >= result[i].pub_date);
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_status_filtering(pool: PgPool) -> Result<()> {
            let success_query = ArticleJoinRowQuery {
                statuses: Some(vec![ArticleStatus::Success]),
                ..Default::default()
            };
            let success_results =
                super::super::search_article_join_rows(Some(success_query), &pool).await?;
            assert!(success_results.len() >= 2);
            for result in &success_results {
                assert_eq!(result.status_code, Some(200));
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_search_patterns"))]
        async fn test_pattern_and_date_filtering(pool: PgPool) -> Result<()> {
            let pub_date_from = Utc.with_ymd_and_hms(2025, 1, 10, 0, 0, 0).unwrap();
            let pub_date_to = Utc.with_ymd_and_hms(2025, 1, 12, 23, 59, 59).unwrap();
            let query = ArticleJoinRowQuery {
                link_pattern: Some("tech.example.com".to_string()),
                pub_date_from: Some(pub_date_from),
                pub_date_to: Some(pub_date_to),
                ..Default::default()
            };
            let results = super::super::search_article_join_rows(Some(query), &pool).await?;
            for result in &results {
                assert!(result.url.contains("tech.example.com"));
                assert!(result.pub_date >= pub_date_from && result.pub_date <= pub_date_to);
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_limit_tests"))]
        async fn test_limit_and_source_filtering(pool: PgPool) -> Result<()> {
            let limit_query = ArticleJoinRowQuery {
                limit: Some(3),
                ..Default::default()
            };
            let limit_results =
                super::super::search_article_join_rows(Some(limit_query), &pool).await?;
            assert!(limit_results.len() <= 3);

            let source_query = ArticleJoinRowQuery {
                source: Some("tech".to_string()),
                ..Default::default()
            };
            let source_results =
                super::super::search_article_join_rows(Some(source_query), &pool).await?;
            for result in &source_results {
                assert_eq!(result.source, "tech");
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_boundary_values"))]
        async fn test_edge_cases_and_boundary_values(pool: PgPool) -> Result<()> {
            let empty_pattern_query = ArticleJoinRowQuery {
                link_pattern: Some("".to_string()),
                ..Default::default()
            };
            let empty_results =
                super::super::search_article_join_rows(Some(empty_pattern_query), &pool).await?;
            let all_results = super::super::search_article_join_rows(None, &pool).await?;
            assert_eq!(empty_results.len(), all_results.len());
            Ok(())
        }
    }

    // 関数名モジュール: search_article_contents
    mod search_article_contents {
        use super::*;

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_basic_content_search(pool: PgPool) -> Result<()> {
            let results = super::super::search_article_contents(None, &pool).await?;
            assert!(results.len() >= 2);
            for i in 1..results.len() {
                assert!(results[i - 1].timestamp >= results[i].timestamp);
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_search_patterns"))]
        async fn test_url_pattern_search(pool: PgPool) -> Result<()> {
            let query = ArticleContentQuery {
                url_pattern: Some("tech.example.com".to_string()),
                ..Default::default()
            };
            let results = super::super::search_article_contents(Some(query), &pool).await?;
            for result in &results {
                assert!(result.url.contains("tech.example.com"));
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_search_patterns"))]
        async fn test_timestamp_range_filtering(pool: PgPool) -> Result<()> {
            let timestamp_from = Utc.with_ymd_and_hms(2025, 1, 10, 0, 0, 0).unwrap();
            let timestamp_to = Utc.with_ymd_and_hms(2025, 1, 12, 23, 59, 59).unwrap();
            let query = ArticleContentQuery {
                timestamp_from: Some(timestamp_from),
                timestamp_to: Some(timestamp_to),
                ..Default::default()
            };
            let results = super::super::search_article_contents(Some(query), &pool).await?;
            for result in &results {
                assert!(result.timestamp >= timestamp_from && result.timestamp <= timestamp_to);
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_status_code_filtering(pool: PgPool) -> Result<()> {
            let success_query = ArticleContentQuery {
                status_code: Some(200),
                ..Default::default()
            };
            let success_results =
                super::super::search_article_contents(Some(success_query), &pool).await?;
            for result in &success_results {
                assert_eq!(result.status_code, 200);
            }
            Ok(())
        }
    }

    // 包括テスト
    mod search_article_join_rows_comprehensive {
        use super::*;

        #[sqlx::test(fixtures("repository_comprehensive"))]
        async fn test_comprehensive_source_filtering(pool: PgPool) -> Result<()> {
            let tech_query = ArticleJoinRowQuery {
                source: Some("tech".to_string()),
                ..Default::default()
            };
            let tech_results =
                super::super::search_article_join_rows(Some(tech_query), &pool).await?;
            assert!(tech_results.len() >= 3);
            for result in &tech_results {
                assert_eq!(result.source, "tech");
            }

            let news_query = ArticleJoinRowQuery {
                source: Some("news".to_string()),
                ..Default::default()
            };
            let news_results =
                super::super::search_article_join_rows(Some(news_query), &pool).await?;
            assert!(news_results.len() >= 2);
            for result in &news_results {
                assert_eq!(result.source, "news");
            }
            Ok(())
        }
    }

    mod search_article_contents_comprehensive {
        use super::*;

        #[sqlx::test(fixtures("repository_comprehensive"))]
        async fn test_comprehensive_content_search(pool: PgPool) -> Result<()> {
            let domain_query = ArticleContentQuery {
                url_pattern: Some("tech-news.com".to_string()),
                ..Default::default()
            };
            let domain_results =
                super::super::search_article_contents(Some(domain_query), &pool).await?;
            assert!(domain_results.len() >= 2);
            for result in &domain_results {
                assert!(result.url.contains("tech-news.com"));
                assert_eq!(result.status_code, 200);
            }

            let error_query = ArticleContentQuery {
                status_code: Some(404),
                ..Default::default()
            };
            let error_results =
                super::super::search_article_contents(Some(error_query), &pool).await?;
            for result in &error_results {
                assert_eq!(result.status_code, 404);
            }
            Ok(())
        }

        #[sqlx::test(fixtures("repository_edge_cases"))]
        async fn test_content_edge_cases(pool: PgPool) -> Result<()> {
            let long_content_query = ArticleContentQuery {
                url_pattern: Some("extremely-long-domain-name".to_string()),
                ..Default::default()
            };
            let long_results =
                super::super::search_article_contents(Some(long_content_query), &pool).await?;
            assert!(long_results.len() >= 1);
            for result in &long_results {
                assert!(result.content.len() > 10000);
            }

            let special_query = ArticleContentQuery {
                url_pattern: Some("unicode.test.com".to_string()),
                ..Default::default()
            };
            let special_results =
                super::super::search_article_contents(Some(special_query), &pool).await?;
            assert!(special_results.len() >= 1);
            for result in &special_results {
                assert!(result.content.contains("SQL injection"));
            }

            let empty_content_query = ArticleContentQuery {
                url_pattern: Some("minimal.test.com".to_string()),
                ..Default::default()
            };
            let empty_results =
                super::super::search_article_contents(Some(empty_content_query), &pool).await?;
            assert!(empty_results.len() >= 1);
            for result in &empty_results {
                assert!(result.content.is_empty());
            }
            Ok(())
        }
    }

    // 関数名モジュール: search_articles（ドメイン向け）
    mod search_articles {
        use super::*;
        use chrono::{Datelike, TimeZone, Utc};

        #[sqlx::test(fixtures("article_basic"))]
        async fn test_basic(pool: PgPool) -> Result<()> {
            let result = super::super::search_articles(None, &pool).await?;
            assert_eq!(result.len(), 6);
            let urls: Vec<&str> = result.iter().map(|a| a.url.as_str()).collect();
            assert!(urls.contains(&"https://example.com/article1"));
            assert!(urls.contains(&"https://example.com/article2"));
            assert!(urls.contains(&"https://example.com/article4"));
            assert!(urls.contains(&"https://example.com/special-chars"));
            assert!(urls.contains(&"https://example.com/empty-title"));
            assert!(urls.contains(&"https://very-long-domain-name-for-testing-url-limits.example.com/very/long/path/to/article"));
            for article in &result {
                assert!(article.updated_at >= article.pub_date);
            }
            Ok(())
        }

        #[sqlx::test(fixtures("article_filter"))]
        async fn test_with_filters(pool: PgPool) -> Result<()> {
            let pub_date_from = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
            let pub_date_to = Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap();

            let query = Some(ArticleQuery {
                link_pattern: Some("tech.example.com".to_string()),
                pub_date_from: Some(pub_date_from),
                pub_date_to: Some(pub_date_to),
                limit: Some(5),
            });
            let articles = super::super::search_articles(query, &pool).await?;
            assert!(articles.len() <= 5);
            assert!(articles.len() >= 1);
            for article in &articles {
                assert!(article.url.contains("tech.example.com"));
                assert!(article.pub_date >= pub_date_from);
                assert!(article.pub_date <= pub_date_to);
            }

            let boundary_query = Some(ArticleQuery {
                link_pattern: Some("tech.example.com".to_string()),
                pub_date_from: Some(pub_date_from),
                pub_date_to: Some(pub_date_to),
                limit: None,
            });
            let boundary_articles = super::super::search_articles(boundary_query, &pool).await?;
            let has_year_start = boundary_articles.iter().any(|a| {
                a.pub_date.year() == 2025 && a.pub_date.month() == 1 && a.pub_date.day() == 1
            });
            let has_year_end = boundary_articles.iter().any(|a| {
                a.pub_date.year() == 2025 && a.pub_date.month() == 12 && a.pub_date.day() == 31
            });
            assert!(has_year_start);
            assert!(has_year_end);

            // limitテスト
            let limit_cases = [Some(0i64), Some(1i64), Some(100i64)];
            for limit in limit_cases {
                let q = Some(ArticleQuery {
                    link_pattern: Some("tech.example.com".to_string()),
                    pub_date_from: None,
                    pub_date_to: None,
                    limit,
                });
                let res = super::super::search_articles(q, &pool).await?;
                match limit.unwrap() {
                    0 => assert_eq!(res.len(), 0),
                    1 => assert!(res.len() <= 1),
                    100 => assert!(res.len() <= 100),
                    _ => unreachable!(),
                }
            }
            Ok(())
        }
    }
}
