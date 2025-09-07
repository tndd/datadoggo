use chrono::{DateTime, Utc};
use sea_query::extension::postgres::PgExpr;
use sea_query::{Cond, Expr, Iden, SelectStatement};

use super::super::types::ArticleStatus;

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
