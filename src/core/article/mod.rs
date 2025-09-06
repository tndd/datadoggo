pub mod model;
pub mod service;

// 公開APIの再エクスポート

// model.rsから
pub use model::{
    count_articles_by_status, count_articles_metadata_by_status, filter_articles_by_status,
    filter_articles_metadata_by_status, format_backlog_articles, format_backlog_articles_metadata,
    Article, ArticleMetadata, ArticleStatus,
};

// repository.rsから（統合後）
pub use service::{
    fetch_and_store_article, fetch_and_store_article_with_client, get_article_content,
    get_article_content_with_client, search_article_contents, search_articles,
    search_backlog_articles_light, store_article_content, ArticleContent, ArticleContentQuery,
    ArticleQuery,
};
