use crate::{
    core::article::{fetch_article_content_via_firecrawl, store_article_content, ArticleContent},
    infra::api::firecrawl::FirecrawlClient,
};
use anyhow::Result;
use sqlx::PgPool;

/// バックログ対象リンクから処理待ちの記事を収集してDBに保存する
pub async fn task_collect_articles<F: FirecrawlClient>(
    firecrawl_client: &F,
    pool: &PgPool,
) -> Result<()> {
    todo!("記事収集処理を実装する")
}

#[cfg(test)]
mod tests {}
