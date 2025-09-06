pub mod model;
pub mod service;

// 公開APIの再エクスポート

// model.rsから（ユーザー向けドメインオブジェクト）
pub use model::{search_articles, Article, ArticleQuery, ArticleStatus};

// service.rsから（内部実装、最小限のみ公開）
pub use service::{
    fetch_and_store_article, fetch_and_store_article_with_client, get_article_content_for_storage,
    get_article_content_for_storage_with_client, store_article_content, ArticleStorageData,
};
