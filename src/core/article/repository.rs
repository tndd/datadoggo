use anyhow::{Context, Result};
use sqlx::PgPool;

use super::model::{
    ArticleContent, ArticleContentQuery, ArticleJoinRow, ArticleJoinRowQuery, ArticleStatus,
    ArticleUrlStatus, ArticleUrlStatusQuery,
};

/// ArticleUrlStatusを取得するリポジトリ関数
pub async fn search_article_url_statuses(
    query: Option<ArticleUrlStatusQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleUrlStatus>> {
    // 入力の正規化
    let query = query.unwrap_or_default();
    let (apply_status, has_unprocessed, has_success, error_codes): (
        bool,
        bool,
        bool,
        Option<Vec<i32>>,
    ) = normalize_statuses(query.statuses.as_deref());

    // 静的SQL（NULL無効化パターン＋適用フラグ）
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

/// ArticleJoinRowを取得するリポジトリ関数
pub async fn search_article_join_rows(
    query: Option<ArticleJoinRowQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleJoinRow>> {
    // 入力の正規化
    let query = query.unwrap_or_default();
    let (apply_status, has_unprocessed, has_success, error_codes): (
        bool,
        bool,
        bool,
        Option<Vec<i32>>,
    ) = normalize_statuses(query.statuses.as_deref());

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

/// ArticleContentを取得するリポジトリ関数
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

// 内部実装：statuses指定の正規化を行う
fn normalize_statuses(statuses: Option<&[ArticleStatus]>) -> (bool, bool, bool, Option<Vec<i32>>) {
    let mut apply = false;
    let mut has_unprocessed = false;
    let mut has_success = false;
    let mut errors: Vec<i32> = Vec::new();

    if let Some(list) = statuses {
        for s in list {
            apply = true;
            match s {
                ArticleStatus::Unprocessed => has_unprocessed = true,
                ArticleStatus::Success => has_success = true,
                ArticleStatus::Error(code) => errors.push(*code),
            }
        }
    }

    let error_codes = if errors.is_empty() {
        None
    } else {
        Some(errors)
    };
    (apply, has_unprocessed, has_success, error_codes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use sqlx::PgPool;

    mod search_article_url_statuses {
        use super::*;
        use crate::core::article::model::ArticleStatus;

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
        use crate::core::article::model::ArticleStatus;

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

        #[sqlx::test(fixtures("store_article_content_logic"))]
        async fn test_basic_insert_and_conflict_resolution(pool: PgPool) -> Result<()> {
            // 新規挿入の確認
            let new_article = ArticleContent {
                url: "https://new.example.com/article".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "New article content".to_string(),
            };
            store_article_content(&new_article, &pool).await?;

            let count = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM articles WHERE url = $1",
                "https://new.example.com/article"
            )
            .fetch_one(&pool)
            .await?;
            assert_eq!(count.unwrap_or(0), 1, "新規記事が挿入されていない");

            // ON CONFLICT時の更新確認
            let updated_article = ArticleContent {
                url: "https://new.example.com/article".to_string(),
                timestamp: Utc::now(),
                status_code: 404,
                content: "Updated content".to_string(),
            };
            store_article_content(&updated_article, &pool).await?;

            let final_count = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM articles WHERE url = $1",
                "https://new.example.com/article"
            )
            .fetch_one(&pool)
            .await?;
            assert_eq!(
                final_count.unwrap_or(0),
                1,
                "UPSERT後に重複レコードが作成された"
            );

            // 更新内容の確認
            let (stored_status, stored_content) = sqlx::query!(
                "SELECT status_code, content FROM articles WHERE url = $1",
                "https://new.example.com/article"
            )
            .fetch_one(&pool)
            .await
            .map(|row| (row.status_code, row.content))?;

            assert_eq!(stored_status, 404, "ステータスコードが更新されていない");
            assert_eq!(
                stored_content, "Updated content",
                "コンテンツが更新されていない"
            );
            Ok(())
        }

        #[sqlx::test(fixtures("store_article_content_logic"))]
        async fn test_distinct_condition_edge_cases(pool: PgPool) -> Result<()> {
            let base_url = "https://distinct-test.example.com/article";

            // ベースライン設定
            let initial_article = ArticleContent {
                url: base_url.to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "Original content".to_string(),
            };
            store_article_content(&initial_article, &pool).await?;

            let initial_timestamp =
                sqlx::query_scalar!("SELECT timestamp FROM articles WHERE url = $1", base_url)
                    .fetch_one(&pool)
                    .await?;

            // 完全に同じ内容での再保存（更新されないはず）
            let same_article = ArticleContent {
                url: base_url.to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "Original content".to_string(),
            };
            store_article_content(&same_article, &pool).await?;

            let timestamp_after_same =
                sqlx::query_scalar!("SELECT timestamp FROM articles WHERE url = $1", base_url)
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(
                initial_timestamp, timestamp_after_same,
                "同じ内容で更新されてしまった"
            );

            // status_codeのみ異なる場合（更新されるはず）
            let status_different = ArticleContent {
                url: base_url.to_string(),
                timestamp: Utc::now(),
                status_code: 404,
                content: "Original content".to_string(),
            };
            store_article_content(&status_different, &pool).await?;

            let status_after_update =
                sqlx::query_scalar!("SELECT status_code FROM articles WHERE url = $1", base_url)
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(
                status_after_update, 404,
                "ステータスコード変更が反映されていない"
            );

            // contentのみ異なる場合（更新されるはず）
            let content_different = ArticleContent {
                url: base_url.to_string(),
                timestamp: Utc::now(),
                status_code: 404,
                content: "Modified content".to_string(),
            };
            store_article_content(&content_different, &pool).await?;

            let content_after_update =
                sqlx::query_scalar!("SELECT content FROM articles WHERE url = $1", base_url)
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(
                content_after_update, "Modified content",
                "コンテンツ変更が反映されていない"
            );
            Ok(())
        }

        #[sqlx::test(fixtures("store_article_content_logic"))]
        async fn test_timestamp_update_behavior(pool: PgPool) -> Result<()> {
            let test_url = "https://timestamp-test.example.com/article";
            let initial_time = Utc::now();

            let initial_article = ArticleContent {
                url: test_url.to_string(),
                timestamp: initial_time,
                status_code: 200,
                content: "Initial content".to_string(),
            };
            store_article_content(&initial_article, &pool).await?;

            let db_timestamp_initial =
                sqlx::query_scalar!("SELECT timestamp FROM articles WHERE url = $1", test_url)
                    .fetch_one(&pool)
                    .await?;

            // 少し待ってから更新（タイムスタンプが変わることを確認）
            std::thread::sleep(std::time::Duration::from_millis(10));

            let updated_article = ArticleContent {
                url: test_url.to_string(),
                timestamp: initial_time, // 入力のタイムスタンプは同じ
                status_code: 404,
                content: "Initial content".to_string(),
            };
            store_article_content(&updated_article, &pool).await?;

            let db_timestamp_after_update =
                sqlx::query_scalar!("SELECT timestamp FROM articles WHERE url = $1", test_url)
                    .fetch_one(&pool)
                    .await?;

            assert_ne!(
                db_timestamp_initial, db_timestamp_after_update,
                "更新時にCURRENT_TIMESTAMPが適用されていない"
            );
            assert!(
                db_timestamp_after_update > db_timestamp_initial,
                "タイムスタンプが逆行している"
            );

            // 同じ内容での再保存ではタイムスタンプが変わらないことを確認
            let no_change_article = ArticleContent {
                url: test_url.to_string(),
                timestamp: Utc::now(),
                status_code: 404,
                content: "Initial content".to_string(),
            };
            store_article_content(&no_change_article, &pool).await?;

            let db_timestamp_after_no_change =
                sqlx::query_scalar!("SELECT timestamp FROM articles WHERE url = $1", test_url)
                    .fetch_one(&pool)
                    .await?;

            assert_eq!(
                db_timestamp_after_update, db_timestamp_after_no_change,
                "内容変更なしでタイムスタンプが更新されてしまった"
            );
            Ok(())
        }

        #[sqlx::test(fixtures("store_article_content_logic"))]
        async fn test_null_value_handling(pool: PgPool) -> Result<()> {
            // スキーマ制約を一時的に緩める（テスト用）
            sqlx::query!("ALTER TABLE articles ALTER COLUMN status_code DROP NOT NULL")
                .execute(&pool)
                .await?;
            sqlx::query!("ALTER TABLE articles ALTER COLUMN content DROP NOT NULL")
                .execute(&pool)
                .await?;

            // 直接SQLでNULL status_codeのレコードを挿入
            let null_status_url = "https://null-status.example.com/article";
            sqlx::query!(
                "INSERT INTO articles (url, timestamp, status_code, content) VALUES ($1, $2, NULL, $3)",
                null_status_url,
                Utc::now(),
                "Content with null status"
            )
            .execute(&pool)
            .await?;

            // NULL status_codeに対して通常の記事を保存（IS DISTINCT FROMの動作確認）
            let normal_article = ArticleContent {
                url: null_status_url.to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "Content with null status".to_string(),
            };
            store_article_content(&normal_article, &pool).await?;

            let updated_status = sqlx::query_scalar!(
                "SELECT status_code FROM articles WHERE url = $1",
                null_status_url
            )
            .fetch_one(&pool)
            .await?;
            assert_eq!(
                updated_status, 200,
                "NULL status_codeが正常値で更新されていない"
            );

            // contentがNULLのケース
            let null_content_url = "https://null-content.example.com/article";
            sqlx::query!(
                "INSERT INTO articles (url, timestamp, status_code, content) VALUES ($1, $2, $3, NULL)",
                null_content_url,
                Utc::now(),
                404
            )
            .execute(&pool)
            .await?;

            let content_update_article = ArticleContent {
                url: null_content_url.to_string(),
                timestamp: Utc::now(),
                status_code: 404,
                content: "Now has content".to_string(),
            };
            store_article_content(&content_update_article, &pool).await?;

            let updated_content = sqlx::query_scalar!(
                "SELECT content FROM articles WHERE url = $1",
                null_content_url
            )
            .fetch_one(&pool)
            .await?;
            assert_eq!(
                updated_content, "Now has content",
                "NULL contentが正常値で更新されていない"
            );

            // NULL vs NULLの場合（更新されないはず）
            let both_null_url = "https://both-null.example.com/article";
            sqlx::query!(
                "INSERT INTO articles (url, timestamp, status_code, content) VALUES ($1, $2, NULL, NULL)",
                both_null_url,
                Utc::now()
            )
            .execute(&pool)
            .await?;

            let initial_timestamp = sqlx::query_scalar!(
                "SELECT timestamp FROM articles WHERE url = $1",
                both_null_url
            )
            .fetch_one(&pool)
            .await?;

            // 再度NULL値で保存を試行
            sqlx::query!(
                "INSERT INTO articles (url, timestamp, status_code, content) VALUES ($1, CURRENT_TIMESTAMP, NULL, NULL)
                 ON CONFLICT (url) DO UPDATE SET 
                     status_code = EXCLUDED.status_code,
                     content = EXCLUDED.content,
                     timestamp = CURRENT_TIMESTAMP
                 WHERE (articles.status_code, articles.content)
                     IS DISTINCT FROM (EXCLUDED.status_code, EXCLUDED.content)",
                both_null_url
            )
            .execute(&pool)
            .await?;

            let final_timestamp = sqlx::query_scalar!(
                "SELECT timestamp FROM articles WHERE url = $1",
                both_null_url
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(
                initial_timestamp, final_timestamp,
                "NULL vs NULLで更新されてしまった"
            );

            // 先にNULL値を削除してから制約を復元
            sqlx::query!("DELETE FROM articles WHERE status_code IS NULL OR content IS NULL")
                .execute(&pool)
                .await?;

            // 制約を元に戻す
            sqlx::query!("ALTER TABLE articles ALTER COLUMN status_code SET NOT NULL")
                .execute(&pool)
                .await?;
            sqlx::query!("ALTER TABLE articles ALTER COLUMN content SET NOT NULL")
                .execute(&pool)
                .await?;

            Ok(())
        }

        #[sqlx::test(fixtures("store_article_content_logic"))]
        async fn test_boundary_values_and_data_integrity(pool: PgPool) -> Result<()> {
            // 空文字列とNULLの区別テスト
            let empty_content_article = ArticleContent {
                url: "https://empty.example.com/article".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "".to_string(),
            };
            store_article_content(&empty_content_article, &pool).await?;

            let stored_empty_content = sqlx::query_scalar!(
                "SELECT content FROM articles WHERE url = $1",
                "https://empty.example.com/article"
            )
            .fetch_one(&pool)
            .await?;
            assert_eq!(stored_empty_content, "", "空文字列が正しく保存されていない");
            assert_ne!(
                stored_empty_content, "null",
                "空文字列がnull文字列として扱われている"
            );

            // status_codeの境界値テスト
            let boundary_status_codes = vec![0, 100, 200, 404, 500, 599, 999];
            for status_code in boundary_status_codes.iter() {
                let boundary_article = ArticleContent {
                    url: format!("https://status-{}.example.com/article", status_code),
                    timestamp: Utc::now(),
                    status_code: *status_code,
                    content: format!("Content for status {}", status_code),
                };
                store_article_content(&boundary_article, &pool).await?;

                let stored_status = sqlx::query_scalar!(
                    "SELECT status_code FROM articles WHERE url = $1",
                    boundary_article.url
                )
                .fetch_one(&pool)
                .await?;
                assert_eq!(
                    stored_status, *status_code,
                    "ステータスコード{}が正しく保存されていない",
                    status_code
                );
            }

            // 極端に長いURLの処理（ただし制約内）
            let long_url = format!(
                "https://very-long-domain-name-for-testing.example.com/{}",
                "a".repeat(200)
            );
            let long_url_article = ArticleContent {
                url: long_url.clone(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "Content for long URL".to_string(),
            };
            store_article_content(&long_url_article, &pool).await?;

            let stored_url =
                sqlx::query_scalar!("SELECT url FROM articles WHERE url = $1", long_url)
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(stored_url, long_url, "長いURLが正しく保存されていない");

            // 特殊文字・Unicode混在コンテンツ（軽くテスト）
            let unicode_article = ArticleContent {
                url: "https://unicode.example.com/test".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "日本語🚀<script>alert('test')</script>\n\ttab".to_string(),
            };
            store_article_content(&unicode_article, &pool).await?;

            let stored_unicode = sqlx::query_scalar!(
                "SELECT content FROM articles WHERE url = $1",
                "https://unicode.example.com/test"
            )
            .fetch_one(&pool)
            .await?;
            assert_eq!(
                stored_unicode, "日本語🚀<script>alert('test')</script>\n\ttab",
                "Unicode混在コンテンツが正しく保存されていない"
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
