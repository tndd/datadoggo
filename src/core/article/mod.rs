pub mod model;
pub mod service;

// 公開APIの再エクスポート

// model.rsから
pub use model::{
    count_article_info_by_status, filter_article_info_by_status, format_backlog_article_info,
    Article, ArticleInfo, ArticleStatus,
};

// service.rsから
pub use service::{
    fetch_and_store_article, fetch_and_store_article_with_client, get_article_content_for_storage,
    get_article_content_for_storage_with_client, search_article_info, search_articles,
    search_backlog_article_info, store_article_content, ArticleInfoQuery, ArticleQuery,
    ArticleStorageData,
};
