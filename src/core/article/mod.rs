pub mod prelude;
pub mod service;

// prelude.rsから（ユーザー向けAPI）
pub use prelude::{search_articles, Article, ArticleQuery};
// service.rsから（内部実装API）
pub use service::{
    fetch_and_store_article, fetch_and_store_article_with_client, get_article_content,
    get_article_content_with_client, search_article_contents, search_article_join_rows,
    search_article_url_statuses, store_article_content, ArticleContent, ArticleContentQuery,
    ArticleJoinRow, ArticleJoinRowQuery, ArticleStatus, ArticleUrlStatus, ArticleUrlStatusQuery,
};
