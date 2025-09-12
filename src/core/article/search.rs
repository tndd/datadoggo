use anyhow::{Context, Result};
use sqlx::PgPool;

use super::model::{Article, ArticleJoinRow, ArticleStatus, ArticleUrlStatus};
use super::query::{ArticleJoinRowQuery, ArticleQuery, ArticleUrlStatusQuery};

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
            if row.status_code == Some(200) && row.content.as_ref().map_or(false, |c| !c.is_empty()) && row.timestamp.is_some() {
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

/// ArticleJoinRowを取得する（読み取り）
async fn search_article_join_rows(
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

// 内部実装：statuses指定の正規化（このモジュール内のみで使用する）
// queryのSQLバインド補助。記事検索系クエリだけで使うためprivateにする。
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

    // normalize_statusesの単体テスト
    // 目的: ステータス指定の正規化ロジックが期待通りであることを確認
    // 検証観点:
    // - applyフラグが指定時にtrueになること
    // - 未処理/成功/エラーコードの各フラグ・配列が正しく反映されること
    mod normalize_statuses {
        use super::*;

        #[test]
        fn test_normalize_statuses_basic() {
            let (apply, unp, ok, errs) = normalize_statuses(Some(&[
                ArticleStatus::Unprocessed,
                ArticleStatus::Success,
                ArticleStatus::Error(404),
                ArticleStatus::Error(500),
            ]));
            assert!(apply);
            assert!(unp);
            assert!(ok);
            assert_eq!(errs.unwrap(), vec![404, 500]);
        }
    }

    mod search_article_url_statuses {
        use super::*;

        /// 基本的なURL状態検索テスト
        /// 目的: 基本的なクエリの動作とURLソートを確認
        #[sqlx::test(fixtures("query_url_status_basic"))]
        async fn test_basic_search(pool: PgPool) -> Result<()> {
            let result = search_article_url_statuses(None, &pool).await?;

            // 3件のarticle_linksが存在することを確認
            assert_eq!(result.len(), 3);

            // URLがソート順（ASC）で取得されることを確認
            assert_eq!(result[0].url, "https://example.com/article-a");
            assert_eq!(result[1].url, "https://example.com/article-b");
            assert_eq!(result[2].url, "https://example.org/post-c");

            // ステータスコードが正しく設定されていることを確認
            assert_eq!(result[0].status_code, Some(200));
            assert_eq!(result[1].status_code, Some(404));
            assert_eq!(result[2].status_code, None); // 未処理

            Ok(())
        }

        /// URLパターンフィルタリングテスト
        /// 目的: ILIKE検索の正常動作を確認
        #[sqlx::test(fixtures("query_url_pattern_filter"))]
        async fn test_url_pattern_filtering(pool: PgPool) -> Result<()> {
            // "tech-blog"パターンで検索
            let query = ArticleUrlStatusQuery {
                url_pattern: Some("tech-blog".to_string()),
                ..Default::default()
            };

            let result = search_article_url_statuses(Some(query), &pool).await?;

            // tech-blog.com の2件のみヒット
            assert_eq!(result.len(), 2);
            assert!(result.iter().all(|r| r.url.contains("tech-blog.com")));
            assert!(result.iter().all(|r| r.status_code == Some(200)));

            Ok(())
        }

        /// ステータスフィルタリングテスト
        /// 目的: ステータス指定検索の正常動作を確認
        #[sqlx::test(fixtures("query_status_filter"))]
        async fn test_status_filtering(pool: PgPool) -> Result<()> {
            // 成功ステータスのみを検索
            let query = ArticleUrlStatusQuery {
                statuses: Some(vec![ArticleStatus::Success]),
                ..Default::default()
            };

            let result = search_article_url_statuses(Some(query), &pool).await?;

            // 2件の成功記事のみヒット
            assert_eq!(result.len(), 2);
            assert!(result.iter().all(|r| r.status_code == Some(200)));

            // 未処理ステータスを検索
            let query = ArticleUrlStatusQuery {
                statuses: Some(vec![ArticleStatus::Unprocessed]),
                ..Default::default()
            };

            let result = search_article_url_statuses(Some(query), &pool).await?;

            // 1件の未処理記事のみヒット
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].status_code, None);
            assert!(result[0].url.contains("unprocessed.com"));

            Ok(())
        }

        /// 制限フィルタリングテスト
        /// 目的: LIMIT句の正常動作を確認
        #[sqlx::test(fixtures("query_url_status_basic"))]
        async fn test_limit_filtering(pool: PgPool) -> Result<()> {
            let query = ArticleUrlStatusQuery {
                limit: Some(2),
                ..Default::default()
            };

            let result = search_article_url_statuses(Some(query), &pool).await?;

            // 制限された件数のみ取得されることを確認
            assert_eq!(result.len(), 2);

            // ソート順序が維持されることを確認（URL ASC）
            assert_eq!(result[0].url, "https://example.com/article-a");
            assert_eq!(result[1].url, "https://example.com/article-b");

            Ok(())
        }
    }

    mod search_article_join_rows {
        use super::*;

        /// 基本的な結合検索テスト
        /// 目的: JOINクエリの基本動作と日付降順ソートを確認
        #[sqlx::test(fixtures("query_join_rows_basic"))]
        async fn test_basic_search(pool: PgPool) -> Result<()> {
            let result = search_article_join_rows(None, &pool).await?;

            // 3件の結合データが取得されることを確認
            assert_eq!(result.len(), 3);

            // 日付の降順ソートを確認（pub_date DESC）
            let pub_dates: Vec<_> = result.iter().map(|r| r.pub_date).collect();
            assert!(pub_dates[0] > pub_dates[1]);
            assert!(pub_dates[1] > pub_dates[2]);

            // データの整合性を確認（最新の記事が最初）
            let first_row = &result[0];
            assert_eq!(first_row.url, "https://latest.com/article");
            assert_eq!(first_row.title, "Latest Article");
            assert_eq!(first_row.source, "news");
            assert_eq!(first_row.status_code, Some(200));

            Ok(())
        }

        /// 日付範囲フィルタリングテスト
        /// 目的: pub_date_from, pub_date_toでの絞り込み動作を確認
        #[sqlx::test(fixtures("query_date_range_filter"))]
        async fn test_date_range_filtering(pool: PgPool) -> Result<()> {
            // 2025年1月の記事のみ検索
            let from_date = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
            let to_date = Utc.with_ymd_and_hms(2025, 1, 31, 23, 59, 59).unwrap();

            let query = ArticleJoinRowQuery {
                pub_date_from: Some(from_date),
                pub_date_to: Some(to_date),
                ..Default::default()
            };

            let result = search_article_join_rows(Some(query), &pool).await?;

            // 2025年1月の3件がヒット
            assert_eq!(result.len(), 3);
            assert!(result
                .iter()
                .all(|r| r.pub_date >= from_date && r.pub_date <= to_date));

            // 日付降順ソートの確認
            assert_eq!(result[0].url, "https://jan31.com/article");
            assert_eq!(result[1].url, "https://jan15.com/article");
            assert_eq!(result[2].url, "https://jan01.com/article");

            Ok(())
        }

        /// ステータスフィルタリングテスト
        /// 目的: ステータス条件による絞り込みを確認
        #[sqlx::test(fixtures("query_status_filter"))]
        async fn test_status_filtering(pool: PgPool) -> Result<()> {
            // 成功ステータスのみを検索
            let query = ArticleJoinRowQuery {
                statuses: Some(vec![ArticleStatus::Success]),
                ..Default::default()
            };

            let result = search_article_join_rows(Some(query), &pool).await?;

            // 2件の成功記事のみヒット
            assert_eq!(result.len(), 2);
            assert!(result.iter().all(|r| r.status_code == Some(200)));
            assert!(result.iter().all(|r| r.content.is_some()));

            // エラーステータスを検索
            let query = ArticleJoinRowQuery {
                statuses: Some(vec![ArticleStatus::Error(404), ArticleStatus::Error(500)]),
                ..Default::default()
            };

            let result = search_article_join_rows(Some(query), &pool).await?;

            // 2件のエラー記事がヒット
            assert_eq!(result.len(), 2);
            assert!(result
                .iter()
                .all(|r| r.status_code == Some(404) || r.status_code == Some(500)));

            Ok(())
        }

        /// ソースフィルタリングテスト
        /// 目的: sourceでの絞り込み動作を確認
        #[sqlx::test(fixtures("query_source_filter"))]
        async fn test_source_filtering(pool: PgPool) -> Result<()> {
            // techソースのみを検索
            let query = ArticleJoinRowQuery {
                source: Some("tech".to_string()),
                ..Default::default()
            };

            let result = search_article_join_rows(Some(query), &pool).await?;

            // 2件のtech記事のみヒット
            assert_eq!(result.len(), 2);
            assert!(result.iter().all(|r| r.source == "tech"));

            // 日付降順ソート確認
            assert_eq!(result[0].url, "https://tech2.com/article");
            assert_eq!(result[1].url, "https://tech1.com/article");

            Ok(())
        }

        /// 制限フィルタリングテスト
        /// 目的: LIMIT句の正常動作を確認
        #[sqlx::test(fixtures("query_join_rows_basic"))]
        async fn test_limit_filtering(pool: PgPool) -> Result<()> {
            let query = ArticleJoinRowQuery {
                limit: Some(2),
                ..Default::default()
            };

            let result = search_article_join_rows(Some(query), &pool).await?;

            // 制限された件数のみ取得されることを確認
            assert_eq!(result.len(), 2);

            // 日付降順で最新の2件が取得されることを確認
            assert_eq!(result[0].url, "https://latest.com/article");
            assert_eq!(result[1].url, "https://middle.com/article");

            Ok(())
        }
    }

    mod search_articles {
        use super::*;

        /// ドメイン向け記事検索の基本テスト
        /// 目的: ArticleJoinRowからArticleへの変換とフィルタリングを確認
        #[sqlx::test(fixtures("query_articles_domain"))]
        async fn test_basic(pool: PgPool) -> Result<()> {
            let result = search_articles(None, &pool).await?;

            // 成功ステータス(200)かつcontentとtimestampが存在する記事のみ取得
            // valid1, valid2のみ（errorとincompleteは除外される）
            assert_eq!(result.len(), 2);

            // 各記事のフィールドが適切に設定されていることを確認
            let first_article = &result[0];
            assert!(first_article.url.starts_with("https://valid"));
            assert!(first_article.title.starts_with("Valid Article"));
            assert!(first_article.content.starts_with("Complete valid content"));

            // 日付の降順ソート（pub_date DESC）を確認
            // valid1が2025-01-02、valid2が2025-01-01なので、valid1が最初
            assert_eq!(result[0].url, "https://valid1.com/article");
            assert_eq!(result[1].url, "https://valid2.com/article");

            Ok(())
        }

        /// フィルタ条件付き検索テスト
        /// 目的: ArticleQueryのフィルタ条件が正常に動作することを確認
        #[sqlx::test(fixtures("query_articles_domain"))]
        async fn test_with_filters(pool: PgPool) -> Result<()> {
            let from_date = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
            let to_date = Utc.with_ymd_and_hms(2025, 1, 1, 23, 59, 59).unwrap();

            let query = ArticleQuery {
                link_pattern: Some("valid".to_string()),
                pub_date_from: Some(from_date),
                pub_date_to: Some(to_date),
                limit: Some(1),
            };

            let result = search_articles(Some(query), &pool).await?;

            // フィルタ条件にマッチする記事のみ取得
            // 2025-01-01の範囲でvalid2のみがヒット
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].url, "https://valid2.com/article");
            assert!(result[0].url.contains("valid"));
            assert!(result[0].pub_date >= from_date && result[0].pub_date <= to_date);

            Ok(())
        }
    }
}
