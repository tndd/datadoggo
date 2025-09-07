use chrono::{DateTime, Utc};
use sea_query::extension::postgres::PgExpr;
use sea_query::{Cond, Expr, Iden, SelectStatement};

use super::types::ArticleStatus;

// テーブル名定義
#[derive(sea_query::Iden, Clone)]
pub enum ArticleLinks {
    Table,
    Url,
    Title,
    PubDate,
    Source,
}

#[derive(sea_query::Iden, Clone)]
pub enum Articles {
    Table,
    Url,
    Timestamp,
    StatusCode,
    Content,
}

// クエリ構築のヘルパー関数群
pub fn apply_url_filter(
    mut select: SelectStatement,
    pattern: Option<&str>,
    table: impl Iden + 'static,
    column: impl Iden + 'static,
) -> SelectStatement {
    if let Some(pattern) = pattern {
        let like_pattern = format!("%{}%", pattern);
        select = select
            .and_where(Expr::col((table, column)).ilike(like_pattern))
            .to_owned();
    }
    select
}

pub fn apply_date_range(
    mut select: SelectStatement,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    table: impl Iden + Clone + 'static,
    column: impl Iden + Clone + 'static,
) -> SelectStatement {
    if let Some(from) = from {
        select = select
            .and_where(Expr::col((table.clone(), column.clone())).gte(from))
            .to_owned();
    }
    if let Some(to) = to {
        select = select
            .and_where(Expr::col((table, column)).lte(to))
            .to_owned();
    }
    select
}

pub fn apply_status_filter(
    mut select: SelectStatement,
    statuses: Option<&[ArticleStatus]>,
) -> SelectStatement {
    if let Some(statuses) = statuses {
        let mut status_cond = Cond::any();
        for status in statuses {
            match status {
                ArticleStatus::Unprocessed => {
                    status_cond = status_cond
                        .add(Expr::col((Articles::Table, Articles::StatusCode)).is_null());
                }
                ArticleStatus::Success => {
                    status_cond =
                        status_cond.add(Expr::col((Articles::Table, Articles::StatusCode)).eq(200));
                }
                ArticleStatus::Error(code) => {
                    status_cond = status_cond
                        .add(Expr::col((Articles::Table, Articles::StatusCode)).eq(*code));
                }
            }
        }
        select = select.cond_where(status_cond).to_owned();
    }
    select
}

pub fn apply_status_code_filter(
    mut select: SelectStatement,
    status_code: Option<i32>,
) -> SelectStatement {
    if let Some(status_code) = status_code {
        select = select
            .and_where(Expr::col(Articles::StatusCode).eq(status_code))
            .to_owned();
    }
    select
}

pub fn apply_optional_limit(mut select: SelectStatement, limit: Option<i64>) -> SelectStatement {
    if let Some(limit) = limit {
        select = select.limit(limit as u64).to_owned();
    }
    select
}

pub fn apply_source_filter(
    mut select: SelectStatement,
    source: Option<&str>,
    table: impl Iden + 'static,
    column: impl Iden + 'static,
) -> SelectStatement {
    if let Some(source) = source {
        select = select
            .and_where(Expr::col((table, column)).eq(source))
            .to_owned();
    }
    select
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use sea_query::{PostgresQueryBuilder, Query};
    use sea_query_binder::SqlxBinder;

    mod apply_url_filter {
        use super::*;

        #[test]
        fn test_with_pattern() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let result = apply_url_filter(
                base_query,
                Some("example.com"),
                Articles::Table,
                Articles::Url,
            );

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(sql.contains("ILIKE"));
            assert!(sql.contains("$1"));
        }

        #[test]
        fn test_without_pattern() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let result = apply_url_filter(base_query, None, Articles::Table, Articles::Url);

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(!sql.contains("ILIKE"));
        }

        #[test]
        fn test_empty_pattern() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let result = apply_url_filter(base_query, Some(""), Articles::Table, Articles::Url);

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(sql.contains("ILIKE"));
            // assert_eq!(values.len(), 1);
            assert!(sql.contains("$1"));
        }
    }

    mod apply_date_range {
        use super::*;

        #[test]
        fn test_with_both_dates() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let from_date = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
            let to_date = Utc.with_ymd_and_hms(2025, 1, 31, 23, 59, 59).unwrap();

            let result = apply_date_range(
                base_query,
                Some(from_date),
                Some(to_date),
                Articles::Table,
                Articles::Timestamp,
            );

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(sql.matches(">=").count() == 1);
            assert!(sql.matches("<=").count() == 1);
            // assert_eq!(values.len(), 2);
        }

        #[test]
        fn test_with_from_only() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let from_date = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();

            let result = apply_date_range(
                base_query,
                Some(from_date),
                None,
                Articles::Table,
                Articles::Timestamp,
            );

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(sql.contains(">="));
            assert!(!sql.contains("<="));
            // assert_eq!(values.len(), 1);
        }

        #[test]
        fn test_with_to_only() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let to_date = Utc.with_ymd_and_hms(2025, 1, 31, 23, 59, 59).unwrap();

            let result = apply_date_range(
                base_query,
                None,
                Some(to_date),
                Articles::Table,
                Articles::Timestamp,
            );

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(!sql.contains(">="));
            assert!(sql.contains("<="));
            // assert_eq!(values.len(), 1);
        }

        #[test]
        fn test_with_no_dates() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let result =
                apply_date_range(base_query, None, None, Articles::Table, Articles::Timestamp);

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(!sql.contains(">="));
            assert!(!sql.contains("<="));
            // assert_eq!(values.len(), 0);
        }
    }

    mod apply_status_filter {
        use super::*;

        #[test]
        fn test_single_status_success() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let statuses = vec![ArticleStatus::Success];
            let result = apply_status_filter(base_query, Some(&statuses));

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(sql.contains("="));
            // assert_eq!(values.len(), 1);
        }

        #[test]
        fn test_single_status_unprocessed() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let statuses = vec![ArticleStatus::Unprocessed];
            let result = apply_status_filter(base_query, Some(&statuses));

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(sql.contains("IS NULL"));
            // assert_eq!(values.len(), 0);
        }

        #[test]
        fn test_single_status_error() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let statuses = vec![ArticleStatus::Error(404)];
            let result = apply_status_filter(base_query, Some(&statuses));

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(sql.contains("="));
            // assert_eq!(values.len(), 1);
        }

        #[test]
        fn test_multiple_statuses() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let statuses = vec![
                ArticleStatus::Success,
                ArticleStatus::Error(404),
                ArticleStatus::Unprocessed,
            ];
            let result = apply_status_filter(base_query, Some(&statuses));

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(sql.contains("="));
            assert!(sql.contains("IS NULL"));
            assert!(sql.contains("OR"));
            // assert_eq!(values.len(), 2); // Success(200) + Error(404)
        }

        #[test]
        fn test_no_statuses() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let result = apply_status_filter(base_query, None);

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(!sql.contains("status_code"));
            // assert_eq!(values.len(), 0);
        }

        #[test]
        fn test_empty_statuses_vec() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let statuses: Vec<ArticleStatus> = vec![];
            let result = apply_status_filter(base_query, Some(&statuses));

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            // 空のベクターの場合、条件は追加されない
            assert!(!sql.contains("status_code"));
            // assert_eq!(values.len(), 0);
        }
    }

    mod apply_status_code_filter {
        use super::*;

        #[test]
        fn test_with_status_code() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let result = apply_status_code_filter(base_query, Some(200));

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(sql.contains("="));
            // assert_eq!(values.len(), 1);
        }

        #[test]
        fn test_without_status_code() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let result = apply_status_code_filter(base_query, None);

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(!sql.contains("status_code"));
            // assert_eq!(values.len(), 0);
        }
    }

    mod apply_optional_limit {
        use super::*;

        #[test]
        fn test_with_limit() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let result = apply_optional_limit(base_query, Some(10));

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(sql.contains("LIMIT"));
        }

        #[test]
        fn test_without_limit() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let result = apply_optional_limit(base_query, None);

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(!sql.contains("LIMIT"));
        }

        #[test]
        fn test_with_zero_limit() {
            let base_query = Query::select().from(Articles::Table).to_owned();
            let result = apply_optional_limit(base_query, Some(0));

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(sql.contains("LIMIT"));
        }
    }

    mod apply_source_filter {
        use super::*;

        #[test]
        fn test_with_source() {
            let base_query = Query::select().from(ArticleLinks::Table).to_owned();
            let result = apply_source_filter(
                base_query,
                Some("tech"),
                ArticleLinks::Table,
                ArticleLinks::Source,
            );

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(sql.contains("="));
            // assert_eq!(values.len(), 1);
        }

        #[test]
        fn test_without_source() {
            let base_query = Query::select().from(ArticleLinks::Table).to_owned();
            let result =
                apply_source_filter(base_query, None, ArticleLinks::Table, ArticleLinks::Source);

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(!sql.contains("source"));
            // assert_eq!(values.len(), 0);
        }

        #[test]
        fn test_empty_source() {
            let base_query = Query::select().from(ArticleLinks::Table).to_owned();
            let result = apply_source_filter(
                base_query,
                Some(""),
                ArticleLinks::Table,
                ArticleLinks::Source,
            );

            let (sql, _) = result.build_sqlx(PostgresQueryBuilder);
            assert!(sql.contains("="));
            // assert_eq!(values.len(), 1);
        }
    }
}
