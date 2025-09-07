use anyhow::{Context, Result};
use sea_query::{Expr, PostgresQueryBuilder, Query};
use sea_query_binder::SqlxBinder;
use sqlx::PgPool;

use super::builders::{
    apply_date_range, apply_optional_limit, apply_source_filter, apply_status_code_filter,
    apply_status_filter, apply_url_filter, ArticleLinks, Articles,
};
use super::types::{
    ArticleContent, ArticleContentQuery, ArticleJoinRow, ArticleJoinRowQuery, ArticleUrlStatus,
    ArticleUrlStatusQuery,
};

/// ArticleUrlStatusを取得するリポジトリ関数
pub async fn search_article_url_statuses(
    query: Option<ArticleUrlStatusQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleUrlStatus>> {
    let query = query.unwrap_or_default();

    let mut select = Query::select()
        .column((ArticleLinks::Table, ArticleLinks::Url))
        .column((Articles::Table, Articles::StatusCode))
        .from(ArticleLinks::Table)
        .left_join(
            Articles::Table,
            Expr::col((ArticleLinks::Table, ArticleLinks::Url))
                .equals((Articles::Table, Articles::Url)),
        )
        .to_owned();

    select = apply_url_filter(
        select,
        query.url_pattern.as_deref(),
        ArticleLinks::Table,
        ArticleLinks::Url,
    );
    select = apply_status_filter(select, query.statuses.as_deref());
    select = select
        .order_by(
            (ArticleLinks::Table, ArticleLinks::Url),
            sea_query::Order::Asc,
        )
        .to_owned();
    select = apply_optional_limit(select, query.limit);

    let (sql, values) = select.build_sqlx(PostgresQueryBuilder);
    let results = sqlx::query_as_with::<_, ArticleUrlStatus, _>(&sql, values)
        .fetch_all(pool)
        .await
        .context("記事URL状態情報の取得に失敗")?;

    Ok(results)
}

/// ArticleJoinRowを取得するリポジトリ関数
pub async fn search_article_join_rows(
    query: Option<ArticleJoinRowQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleJoinRow>> {
    let query = query.unwrap_or_default();

    let mut select = Query::select()
        .column((ArticleLinks::Table, ArticleLinks::Url))
        .column((ArticleLinks::Table, ArticleLinks::Title))
        .column((ArticleLinks::Table, ArticleLinks::PubDate))
        .column((ArticleLinks::Table, ArticleLinks::Source))
        .column((Articles::Table, Articles::Timestamp))
        .column((Articles::Table, Articles::StatusCode))
        .column((Articles::Table, Articles::Content))
        .from(ArticleLinks::Table)
        .left_join(
            Articles::Table,
            Expr::col((ArticleLinks::Table, ArticleLinks::Url))
                .equals((Articles::Table, Articles::Url)),
        )
        .to_owned();

    select = apply_url_filter(
        select,
        query.link_pattern.as_deref(),
        ArticleLinks::Table,
        ArticleLinks::Url,
    );
    select = apply_date_range(
        select,
        query.pub_date_from,
        query.pub_date_to,
        ArticleLinks::Table,
        ArticleLinks::PubDate,
    );
    select = apply_status_filter(select, query.statuses.as_deref());
    select = apply_source_filter(
        select,
        query.source.as_deref(),
        ArticleLinks::Table,
        ArticleLinks::Source,
    );
    select = select
        .order_by(
            (ArticleLinks::Table, ArticleLinks::PubDate),
            sea_query::Order::Desc,
        )
        .to_owned();
    select = apply_optional_limit(select, query.limit);

    let (sql, values) = select.build_sqlx(PostgresQueryBuilder);
    let results = sqlx::query_as_with::<_, ArticleJoinRow, _>(&sql, values)
        .fetch_all(pool)
        .await
        .context("記事結合情報の取得に失敗")?;

    Ok(results)
}

/// ArticleContentを取得するリポジトリ関数
pub async fn search_article_contents(
    query: Option<ArticleContentQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleContent>> {
    let query = query.unwrap_or_default();

    let mut select = Query::select()
        .column(Articles::Url)
        .column(Articles::Timestamp)
        .column(Articles::StatusCode)
        .column(Articles::Content)
        .from(Articles::Table)
        .to_owned();

    select = apply_url_filter(
        select,
        query.url_pattern.as_deref(),
        Articles::Table,
        Articles::Url,
    );
    select = apply_date_range(
        select,
        query.timestamp_from,
        query.timestamp_to,
        Articles::Table,
        Articles::Timestamp,
    );
    select = apply_status_code_filter(select, query.status_code);
    select = select
        .order_by(Articles::Timestamp, sea_query::Order::Desc)
        .to_owned();

    let (sql, values) = select.build_sqlx(PostgresQueryBuilder);
    let articles = sqlx::query_as_with::<_, ArticleContent, _>(&sql, values)
        .fetch_all(pool)
        .await?;

    Ok(articles)
}

/// 記事内容をデータベースに保存する
/// 重複した場合には更新を行う
pub async fn store_article_content(article: &ArticleContent, pool: &PgPool) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO articles (url, status_code, content)
        VALUES ($1, $2, $3)
        ON CONFLICT (url) DO UPDATE SET 
            status_code = EXCLUDED.status_code,
            content = EXCLUDED.content,
            timestamp = CURRENT_TIMESTAMP
        WHERE (articles.status_code, articles.content)
            IS DISTINCT FROM (EXCLUDED.status_code, EXCLUDED.content)
        "#,
        article.url,
        article.status_code,
        article.content
    )
    .execute(pool)
    .await
    .context("記事データのデータベース保存に失敗")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use sqlx::PgPool;

    mod search_article_url_statuses {
        use super::*;
        use crate::core::article::types::ArticleStatus;

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_basic_search(pool: PgPool) -> Result<()> {
            let results = search_article_url_statuses(None, &pool).await?;
            assert!(results.len() >= 3, "基本検索で十分な件数が取得されていない");

            // URL順ソートの確認
            for i in 1..results.len() {
                assert!(
                    results[i - 1].url <= results[i].url,
                    "結果がURL順にソートされていない"
                );
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_status_filtering(pool: PgPool) -> Result<()> {
            // 成功記事のみ取得
            let success_query = ArticleUrlStatusQuery {
                statuses: Some(vec![ArticleStatus::Success]),
                ..Default::default()
            };
            let success_results = search_article_url_statuses(Some(success_query), &pool).await?;

            for result in &success_results {
                assert_eq!(result.status_code, Some(200), "成功記事以外が含まれている");
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_search_patterns"))]
        async fn test_url_pattern_filtering(pool: PgPool) -> Result<()> {
            // 特定ドメインの検索
            let domain_query = ArticleUrlStatusQuery {
                url_pattern: Some("example.com".to_string()),
                ..Default::default()
            };
            let domain_results = search_article_url_statuses(Some(domain_query), &pool).await?;

            for result in &domain_results {
                assert!(
                    result.url.contains("example.com"),
                    "URLパターンが一致しない: {}",
                    result.url
                );
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_limit_tests"))]
        async fn test_limit_filtering(pool: PgPool) -> Result<()> {
            let limit_query = ArticleUrlStatusQuery {
                limit: Some(2),
                ..Default::default()
            };
            let limit_results = search_article_url_statuses(Some(limit_query), &pool).await?;

            assert!(
                limit_results.len() <= 2,
                "limit=2で2件を超える結果が返された"
            );
            Ok(())
        }
    }

    mod search_article_join_rows {
        use super::*;
        use crate::core::article::types::ArticleStatus;

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_basic_search(pool: PgPool) -> Result<()> {
            let result = search_article_join_rows(None, &pool).await?;
            assert!(result.len() >= 1, "基本検索で最低1件の結果が必要");

            // 日付順ソート（DESC）の確認
            for i in 1..result.len() {
                assert!(
                    result[i - 1].pub_date >= result[i].pub_date,
                    "結果が日付順（降順）にソートされていない"
                );
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_status_filtering(pool: PgPool) -> Result<()> {
            // 成功記事のみ取得
            let success_query = ArticleJoinRowQuery {
                statuses: Some(vec![ArticleStatus::Success]),
                ..Default::default()
            };
            let success_results = search_article_join_rows(Some(success_query), &pool).await?;

            assert!(success_results.len() >= 2, "成功記事が期待件数取得できない");
            for result in &success_results {
                assert_eq!(result.status_code, Some(200), "成功記事以外が含まれている");
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

            let results = search_article_join_rows(Some(query), &pool).await?;

            for result in &results {
                assert!(
                    result.url.contains("tech.example.com"),
                    "URLパターンが一致しない: {}",
                    result.url
                );
                assert!(
                    result.pub_date >= pub_date_from && result.pub_date <= pub_date_to,
                    "日付範囲外の結果が含まれている: {}",
                    result.pub_date
                );
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_limit_tests"))]
        async fn test_limit_and_source_filtering(pool: PgPool) -> Result<()> {
            // limitテスト
            let limit_query = ArticleJoinRowQuery {
                limit: Some(3),
                ..Default::default()
            };
            let limit_results = search_article_join_rows(Some(limit_query), &pool).await?;
            assert!(
                limit_results.len() <= 3,
                "limit=3で3件を超える結果が返された"
            );

            // sourceフィルターテスト
            let source_query = ArticleJoinRowQuery {
                source: Some("tech".to_string()),
                ..Default::default()
            };
            let source_results = search_article_join_rows(Some(source_query), &pool).await?;

            for result in &source_results {
                assert_eq!(result.source, "tech", "ソースフィルターが機能していない");
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_boundary_values"))]
        async fn test_edge_cases_and_boundary_values(pool: PgPool) -> Result<()> {
            // 空パターンテスト
            let empty_pattern_query = ArticleJoinRowQuery {
                link_pattern: Some("".to_string()),
                ..Default::default()
            };
            let empty_results = search_article_join_rows(Some(empty_pattern_query), &pool).await?;

            // 空パターンは全てにマッチするはず
            let all_results = search_article_join_rows(None, &pool).await?;
            assert_eq!(
                empty_results.len(),
                all_results.len(),
                "空パターンで全結果が取得されていない"
            );
            Ok(())
        }
    }

    mod search_article_contents {
        use super::*;

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_basic_content_search(pool: PgPool) -> Result<()> {
            let results = search_article_contents(None, &pool).await?;
            assert!(
                results.len() >= 2,
                "基本検索で十分な記事内容が取得されていない"
            );

            // timestamp順ソート（DESC）の確認
            for i in 1..results.len() {
                assert!(
                    results[i - 1].timestamp >= results[i].timestamp,
                    "結果がタイムスタンプ順（降順）にソートされていない"
                );
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_search_patterns"))]
        async fn test_url_pattern_search(pool: PgPool) -> Result<()> {
            let query = ArticleContentQuery {
                url_pattern: Some("tech.example.com".to_string()),
                ..Default::default()
            };
            let results = search_article_contents(Some(query), &pool).await?;

            for result in &results {
                assert!(
                    result.url.contains("tech.example.com"),
                    "URLパターンが一致しない"
                );
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
            let results = search_article_contents(Some(query), &pool).await?;

            for result in &results {
                assert!(
                    result.timestamp >= timestamp_from && result.timestamp <= timestamp_to,
                    "タイムスタンプ範囲外の結果が含まれている"
                );
            }
            Ok(())
        }

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_status_code_filtering(pool: PgPool) -> Result<()> {
            // 成功記事のみ
            let success_query = ArticleContentQuery {
                status_code: Some(200),
                ..Default::default()
            };
            let success_results = search_article_contents(Some(success_query), &pool).await?;

            for result in &success_results {
                assert_eq!(result.status_code, 200, "status_code=200以外が含まれている");
            }
            Ok(())
        }
    }

    mod store_article_content {
        use super::*;

        #[sqlx::test]
        async fn test_basic_storage(pool: PgPool) -> Result<()> {
            let now = Utc::now();
            let test_article = ArticleContent {
                url: "https://test.example.com/basic".to_string(),
                timestamp: now,
                status_code: 200,
                content: "# Basic Test Article\n\nThis is basic test content.".to_string(),
            };
            store_article_content(&test_article, &pool).await?;

            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;
            assert!(count.unwrap_or(0) >= 1, "記事が保存されていない");
            Ok(())
        }

        #[sqlx::test]
        async fn test_duplicate_handling(pool: PgPool) -> Result<()> {
            let now = Utc::now();
            let original_article = ArticleContent {
                url: "https://test.example.com/duplicate".to_string(),
                timestamp: now,
                status_code: 200,
                content: "Original content".to_string(),
            };
            store_article_content(&original_article, &pool).await?;

            let updated_article = ArticleContent {
                url: "https://test.example.com/duplicate".to_string(),
                timestamp: now,
                status_code: 200,
                content: "Updated content".to_string(),
            };
            store_article_content(&updated_article, &pool).await?;

            let count = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM articles WHERE url = $1",
                "https://test.example.com/duplicate"
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(count.unwrap_or(0), 1, "重複記事が複数保存されている");

            // 内容が更新されているかを確認
            let stored_content = sqlx::query_scalar!(
                "SELECT content FROM articles WHERE url = $1",
                "https://test.example.com/duplicate"
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(
                stored_content, "Updated content",
                "記事内容が更新されていない"
            );
            Ok(())
        }

        #[sqlx::test]
        async fn test_large_content_storage(pool: PgPool) -> Result<()> {
            let large_content = "A".repeat(100000); // 100KB のコンテンツ
            let test_article = ArticleContent {
                url: "https://test.example.com/large".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: large_content.clone(),
            };

            store_article_content(&test_article, &pool).await?;

            let stored_content = sqlx::query_scalar!(
                "SELECT content FROM articles WHERE url = $1",
                "https://test.example.com/large"
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(
                stored_content.len(),
                100000,
                "大容量コンテンツが正しく保存されていない"
            );
            Ok(())
        }

        #[sqlx::test]
        async fn test_special_characters(pool: PgPool) -> Result<()> {
            let special_content = "特殊文字テスト: éñüñ, 🚀, \"quotes\", <tags>, & entities";
            let test_article = ArticleContent {
                url: "https://test.example.com/special-chars".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: special_content.to_string(),
            };

            store_article_content(&test_article, &pool).await?;

            let stored_content = sqlx::query_scalar!(
                "SELECT content FROM articles WHERE url = $1",
                "https://test.example.com/special-chars"
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(
                stored_content, special_content,
                "特殊文字が正しく保存されていない"
            );
            Ok(())
        }

        #[sqlx::test]
        async fn test_no_update_when_same_content(pool: PgPool) -> Result<()> {
            let test_article = ArticleContent {
                url: "https://test.example.com/same".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "Same content".to_string(),
            };

            // 最初の保存
            store_article_content(&test_article, &pool).await?;
            let first_timestamp = sqlx::query_scalar!(
                "SELECT timestamp FROM articles WHERE url = $1",
                "https://test.example.com/same"
            )
            .fetch_one(&pool)
            .await?;

            // 同じ内容で再保存（タイムスタンプは更新されないはず）
            store_article_content(&test_article, &pool).await?;
            let second_timestamp = sqlx::query_scalar!(
                "SELECT timestamp FROM articles WHERE url = $1",
                "https://test.example.com/same"
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(
                first_timestamp, second_timestamp,
                "同じ内容で再保存時にタイムスタンプが更新されてしまった"
            );
            Ok(())
        }

        #[sqlx::test(fixtures("repository_edge_cases"))]
        async fn test_extreme_content_storage(pool: PgPool) -> Result<()> {
            // 極端に長いコンテンツの保存テスト
            let huge_content = "Very long content: ".to_string() + &"A".repeat(500000); // 500KB
            let huge_article = ArticleContent {
                url: "https://test-huge.example.com/massive".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: huge_content.clone(),
            };

            store_article_content(&huge_article, &pool).await?;

            let stored_content = sqlx::query_scalar!(
                "SELECT content FROM articles WHERE url = $1",
                "https://test-huge.example.com/massive"
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(
                stored_content.len(),
                huge_content.len(),
                "大容量コンテンツが正しく保存されていない"
            );
            Ok(())
        }

        #[sqlx::test]
        async fn test_unicode_and_special_characters_storage(pool: PgPool) -> Result<()> {
            // Unicode文字、絵文字、制御文字等のテスト（NULL文字は除外）
            let complex_content = "🚀 Unicode test: 日本語 中文 한글 العربية\n\t改行とタブ\r\n制御文字テスト\u{1F4A9}\u{200D}\u{2642}\u{FE0F}";
            let unicode_article = ArticleContent {
                url: "https://unicode-test.example.com/complex".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: complex_content.to_string(),
            };

            store_article_content(&unicode_article, &pool).await?;

            let stored_content = sqlx::query_scalar!(
                "SELECT content FROM articles WHERE url = $1",
                "https://unicode-test.example.com/complex"
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(
                stored_content, complex_content,
                "Unicode文字が正しく保存されていない"
            );
            Ok(())
        }

        #[sqlx::test]
        async fn test_sql_injection_protection(pool: PgPool) -> Result<()> {
            // SQLインジェクション対策のテスト
            let malicious_content = "'; DROP TABLE articles; SELECT 'hacked";
            let malicious_url = "https://hack.test.com'; DROP TABLE articles; --";

            let injection_article = ArticleContent {
                url: malicious_url.to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: malicious_content.to_string(),
            };

            // この操作が成功し、テーブルが削除されないことを確認
            store_article_content(&injection_article, &pool).await?;

            // テーブルがまだ存在することを確認
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;

            assert!(
                count.unwrap_or(0) > 0,
                "SQLインジェクションが成功してしまった可能性"
            );

            // 保存されたコンテンツが正しくエスケープされていることを確認
            let stored_content =
                sqlx::query_scalar!("SELECT content FROM articles WHERE url = $1", malicious_url)
                    .fetch_one(&pool)
                    .await?;

            assert_eq!(
                stored_content, malicious_content,
                "SQLインジェクション対策後のコンテンツが正しくない"
            );
            Ok(())
        }
    }

    mod search_article_join_rows_comprehensive {
        use super::*;

        #[sqlx::test(fixtures("repository_comprehensive"))]
        async fn test_comprehensive_source_filtering(pool: PgPool) -> Result<()> {
            // tech ソースのみ取得
            let tech_query = ArticleJoinRowQuery {
                source: Some("tech".to_string()),
                ..Default::default()
            };
            let tech_results = search_article_join_rows(Some(tech_query), &pool).await?;

            assert!(
                tech_results.len() >= 3,
                "tech ソースの記事が期待件数取得できない"
            );
            for result in &tech_results {
                assert_eq!(result.source, "tech", "tech以外のソースが含まれている");
            }

            // news ソースのみ取得
            let news_query = ArticleJoinRowQuery {
                source: Some("news".to_string()),
                ..Default::default()
            };
            let news_results = search_article_join_rows(Some(news_query), &pool).await?;

            assert!(
                news_results.len() >= 2,
                "news ソースの記事が期待件数取得できない"
            );
            for result in &news_results {
                assert_eq!(result.source, "news", "news以外のソースが含まれている");
            }
            Ok(())
        }
    }

    mod search_article_contents_comprehensive {
        use super::*;

        #[sqlx::test(fixtures("repository_comprehensive"))]
        async fn test_comprehensive_content_search(pool: PgPool) -> Result<()> {
            // 特定ドメインのコンテンツ検索
            let domain_query = ArticleContentQuery {
                url_pattern: Some("tech-news.com".to_string()),
                ..Default::default()
            };
            let domain_results = search_article_contents(Some(domain_query), &pool).await?;

            assert!(
                domain_results.len() >= 2,
                "ドメイン検索でコンテンツが期待件数取得できない"
            );
            for result in &domain_results {
                assert!(
                    result.url.contains("tech-news.com"),
                    "期待外のドメインが含まれている"
                );
                assert_eq!(result.status_code, 200, "成功記事以外が含まれている");
            }

            // 様々なステータスコードでの検索
            let error_query = ArticleContentQuery {
                status_code: Some(404),
                ..Default::default()
            };
            let error_results = search_article_contents(Some(error_query), &pool).await?;

            for result in &error_results {
                assert_eq!(result.status_code, 404, "404以外のステータスが含まれている");
            }
            Ok(())
        }

        #[sqlx::test(fixtures("repository_edge_cases"))]
        async fn test_content_edge_cases(pool: PgPool) -> Result<()> {
            // 極端に長いコンテンツを持つ記事の取得
            let long_content_query = ArticleContentQuery {
                url_pattern: Some("extremely-long-domain-name".to_string()),
                ..Default::default()
            };
            let long_results = search_article_contents(Some(long_content_query), &pool).await?;

            assert!(
                long_results.len() >= 1,
                "長いコンテンツ検索が機能していない"
            );
            for result in &long_results {
                assert!(
                    result.content.len() > 10000,
                    "期待するほど長いコンテンツではない"
                );
            }

            // 特殊文字を含むコンテンツの検索
            let special_query = ArticleContentQuery {
                url_pattern: Some("unicode.test.com".to_string()),
                ..Default::default()
            };
            let special_results = search_article_contents(Some(special_query), &pool).await?;

            assert!(
                special_results.len() >= 1,
                "特殊文字コンテンツ検索が機能していない"
            );
            for result in &special_results {
                assert!(
                    result.content.contains("SQL injection"),
                    "期待する特殊文字コンテンツでない"
                );
            }

            // 空のコンテンツでの検索
            let empty_content_query = ArticleContentQuery {
                url_pattern: Some("minimal.test.com".to_string()),
                ..Default::default()
            };
            let empty_results = search_article_contents(Some(empty_content_query), &pool).await?;

            assert!(empty_results.len() >= 1, "空コンテンツ検索が機能していない");
            for result in &empty_results {
                assert!(result.content.is_empty(), "期待する空コンテンツではない");
            }
            Ok(())
        }
    }
}
