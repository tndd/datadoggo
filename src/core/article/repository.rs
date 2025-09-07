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
