pub mod model;
pub mod repository;
pub mod service;

// 公開APIの再エクスポート

// model.rsから
pub use model::{
    count_articles_by_status, count_articles_metadata_by_status, filter_articles_by_status,
    filter_articles_metadata_by_status, format_backlog_articles, format_backlog_articles_metadata,
    Article, ArticleMetadata, ArticleStatus,
};

// repository.rsから
pub use repository::{
    search_article_contents, search_articles, search_backlog_articles_light, store_article_content,
    ArticleContent, ArticleContentQuery, ArticleQuery,
};

// service.rsから
pub use service::{get_article_content, get_article_content_with_client};
