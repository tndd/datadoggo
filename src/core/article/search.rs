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
    use crate::core::article::model::ArticleStatus;
    use crate::core::article::query::{ArticleJoinRowQuery, ArticleQuery, ArticleUrlStatusQuery};
    use chrono::{DateTime, Utc};
    use sqlx::PgPool;

    // ヘルパー系は helper モジュールに集約
    mod helper {
        use super::*;

        /// URLだけの配列に変換（可読性向上用）
        pub fn get_urls<T, F>(items: &[T], f: F) -> Vec<String>
        where
            F: Fn(&T) -> &str,
        {
            items.iter().map(|x| f(x).to_string()).collect()
        }

        /// 期間指定のためのユーティリティ
        pub fn ts(s: &str) -> DateTime<Utc> {
            s.parse::<DateTime<Utc>>().expect("RFC3339に準拠した日時")
        }
    }

    // 内部関数 normalize_statuses の検証
    mod normalize_statuses {
        use super::normalize_statuses;
        use super::*;

        /// 目的: None入力時にフィルタが適用されないことを確認
        /// 検証観点: apply=false, 他フラグ=false, エラーNone
        #[test]
        fn test_none() {
            let (apply, has_unprocessed, has_success, errors) = normalize_statuses(None);
            assert!(!apply);
            assert!(!has_unprocessed);
            assert!(!has_success);
            assert!(errors.is_none());
        }

        /// 目的: 複合指定時のフラグ・エラーコード展開を確認
        /// 検証観点: apply=true, 各フラグの正当性, エラー配列の順序保持
        #[test]
        fn test_mixed() {
            let statuses = vec![
                ArticleStatus::Unprocessed,
                ArticleStatus::Success,
                ArticleStatus::Error(500),
                ArticleStatus::Error(404),
            ];
            let (apply, unp, ok, errs) = normalize_statuses(Some(&statuses));
            assert!(apply);
            assert!(unp);
            assert!(ok);
            assert_eq!(errs, Some(vec![500, 404]));
        }

        /// 目的: 空スライス指定時にフィルタ非適用となることを確認
        /// 検証観点: apply=false（ループ未突入のため）
        #[test]
        fn test_empty_slice() {
            let empty: Vec<ArticleStatus> = vec![];
            let (apply, unp, ok, errs) = normalize_statuses(Some(&empty));
            assert!(!apply);
            assert!(!unp);
            assert!(!ok);
            assert!(errs.is_none());
        }
    }

    // DB使用テスト: search_article_url_statuses
    mod search_article_url_statuses {
        use super::helper::*;
        use super::*;

        /// 目的: 汎用fixtureからURL状態を一括取得し、NULL/200/エラーが混在して返ることを確認
        /// 検証観点:
        /// - パターン未指定で全件取得
        /// - Left Join のため未処理(Null)が混ざる
        /// - 件数と代表的なURLの存在確認
        #[sqlx::test(fixtures("search"))]
        async fn test_basic_fetch(pool: PgPool) -> Result<(), anyhow::Error> {
            let results = search_article_url_statuses(None, &pool).await?;
            assert!(results.len() >= 6, "最低6件は投入されている前提");

            // 代表値の存在チェック
            let urls = get_urls(&results, |x| &x.url);
            assert!(urls.iter().any(|u| u.contains("example.com/article-1"))); // 200
            assert!(urls.iter().any(|u| u.contains("example.com/article-2"))); // error
            assert!(urls.iter().any(|u| u.contains("example.com/article-3"))); // unprocessed

            Ok(())
        }

        /// 目的: ステータスフィルタ（未処理/成功/特定エラー）が正しく適用されることを検証
        /// 検証観点: normalize_statusesのSQLバインド効果をE2Eで確認
        #[sqlx::test(fixtures("search_status_filter"))]
        async fn test_status_filters(pool: PgPool) -> Result<(), anyhow::Error> {
            // 未処理のみ
            let q = ArticleUrlStatusQuery {
                url_pattern: None,
                statuses: Some(vec![ArticleStatus::Unprocessed]),
                limit: None,
            };
            let unprocessed = search_article_url_statuses(Some(q), &pool).await?;
            assert_eq!(unprocessed.len(), 1);
            assert_eq!(unprocessed[0].status_code, None);

            // 成功のみ
            let q = ArticleUrlStatusQuery {
                url_pattern: None,
                statuses: Some(vec![ArticleStatus::Success]),
                limit: None,
            };
            let success = search_article_url_statuses(Some(q), &pool).await?;
            assert_eq!(success.len(), 1);
            assert_eq!(success[0].status_code, Some(200));

            // 特定エラーのみ（404）
            let q = ArticleUrlStatusQuery {
                url_pattern: None,
                statuses: Some(vec![ArticleStatus::Error(404)]),
                limit: None,
            };
            let not_found = search_article_url_statuses(Some(q), &pool).await?;
            assert_eq!(not_found.len(), 1);
            assert_eq!(not_found[0].status_code, Some(404));
            Ok(())
        }

        /// 目的: URLパターン・Limitの適用と並び順(ASC by url)を確認
        /// 検証観点: ILIKE + LIMIT + ORDER BYの組み合わせ
        #[sqlx::test(fixtures("search_limit"))]
        async fn test_pattern_and_limit(pool: PgPool) -> Result<(), anyhow::Error> {
            let q = ArticleUrlStatusQuery {
                url_pattern: Some("https://limit.test".to_string()),
                statuses: None,
                limit: Some(2),
            };
            let results = search_article_url_statuses(Some(q), &pool).await?;
            assert_eq!(results.len(), 2);
            let urls = get_urls(&results, |x| &x.url);
            assert_eq!(
                urls,
                vec![
                    "https://limit.test/a".to_string(),
                    "https://limit.test/b".to_string(),
                ]
            );
            Ok(())
        }
    }

    // DB使用テスト: search_articles（ドメインArticleを返すフィルタ付き）
    mod search_articles {
        use super::helper::*;
        use super::*;

        /// 目的: 成功(200)かつcontent非空・timestamp有りのみが返ること
        /// 検証観点: JOIN -> フィルタ -> Article変換 の一連の正当性
        #[sqlx::test(fixtures("search"))]
        async fn test_basic_filter_and_transform(pool: PgPool) -> Result<(), anyhow::Error> {
            let results = search_articles(None, &pool).await?;
            // search.sql には 200&content有り が3件以上含まれる前提
            assert!(results.len() >= 3);

            // いずれもcontent非空・URL妥当性
            assert!(results.iter().all(|a| !a.content.is_empty()));
            assert!(results
                .iter()
                .any(|a| a.url.contains("example.com/article-1")));
            assert!(results
                .iter()
                .any(|a| a.url.contains("another.com/path/ok")));
            Ok(())
        }

        /// 目的: 空文字contentは除外されること
        /// 検証観点: content.is_empty() の除外ロジック
        #[sqlx::test(fixtures("search_empty_content"))]
        async fn test_drop_empty_content(pool: PgPool) -> Result<(), anyhow::Error> {
            let results = search_articles(None, &pool).await?;
            let urls = get_urls(&results, |x| &x.url);
            assert!(urls.contains(&"https://empty.test/ok".to_string()));
            assert!(!urls.contains(&"https://empty.test/empty".to_string()));
            Ok(())
        }

        /// 目的: link_pattern・期間・limitの複合指定が正しく作用
        /// 検証観点: ArticleJoinRowQueryへの変換ロジック含め E2E 確認
        #[sqlx::test(fixtures("search"))]
        async fn test_pattern_date_limit(pool: PgPool) -> Result<(), anyhow::Error> {
            let q = ArticleQuery {
                link_pattern: Some("another.com".to_string()),
                pub_date_from: Some(ts("2025-02-01T00:00:00Z")),
                pub_date_to: Some(ts("2025-02-01T23:59:59Z")),
                // 注意: limitはJOIN前に適用されるため、ここでは指定しない
                limit: None,
            };
            let results = search_articles(Some(q), &pool).await?;
            assert!(results.len() >= 1);
            assert!(results[0].url.contains("another.com/path/ok"));
            Ok(())
        }
    }

    // DB使用テスト: search_article_join_rows（低レベルJOIN結果）
    mod search_article_join_rows {
        use super::helper::*;
        use super::search_article_join_rows;
        use super::*;

        /// 目的: ステータス複合指定（Success + Error）で該当行のみ返る
        /// 検証観点: normalize_statuses -> SQL条件のOR結合が正しく動作
        #[sqlx::test(fixtures("search"))]
        async fn test_status_mixed(pool: PgPool) -> Result<(), anyhow::Error> {
            let q = ArticleJoinRowQuery {
                link_pattern: None,
                pub_date_from: None,
                pub_date_to: None,
                statuses: Some(vec![ArticleStatus::Success, ArticleStatus::Error(500)]),
                source: None,
                limit: None,
            };
            let rows = search_article_join_rows(Some(q), &pool).await?;
            assert!(rows
                .iter()
                .all(|r| r.status_code == Some(200) || r.status_code == Some(500)));
            assert!(rows.iter().any(|r| r.status_code == Some(200)));
            assert!(rows.iter().any(|r| r.status_code == Some(500)));
            Ok(())
        }

        /// 目的: sourceフィルタで特定ソースのみ抽出できる
        /// 検証観点: l.source = $8 の等価フィルタ
        #[sqlx::test(fixtures("search_source_filter"))]
        async fn test_source_filter(pool: PgPool) -> Result<(), anyhow::Error> {
            let q = ArticleJoinRowQuery {
                link_pattern: None,
                pub_date_from: None,
                pub_date_to: None,
                statuses: Some(vec![ArticleStatus::Success]),
                source: Some("rss".to_string()),
                limit: None,
            };
            let rows = search_article_join_rows(Some(q), &pool).await?;
            assert!(!rows.is_empty());
            assert!(rows.iter().all(|r| r.source == "rss"));
            Ok(())
        }

        /// 目的: 期間境界が含まれる（>=, <=）ことを確認
        /// 検証観点: pub_date_from/toのinclusive動作
        #[sqlx::test(fixtures("search_pub_date_range"))]
        async fn test_date_range_inclusive(pool: PgPool) -> Result<(), anyhow::Error> {
            let q = ArticleJoinRowQuery {
                link_pattern: None,
                pub_date_from: Some(ts("2025-01-01T00:00:00Z")),
                pub_date_to: Some(ts("2025-01-01T23:59:59Z")),
                statuses: Some(vec![ArticleStatus::Success]),
                source: None,
                limit: None,
            };
            let rows = search_article_join_rows(Some(q), &pool).await?;
            let urls = get_urls(&rows, |x| &x.url);
            assert!(urls.contains(&"https://date.test/exact-start".to_string()));
            assert!(urls.contains(&"https://date.test/exact-end".to_string()));
            assert!(!urls.contains(&"https://date.test/before".to_string()));
            Ok(())
        }
    }
}
