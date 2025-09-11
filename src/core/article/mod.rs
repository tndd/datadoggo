pub mod builders;
pub mod model;
pub mod prelude;
pub mod repository;
pub mod service;

// prelude.rsから（ユーザー向けAPI）
pub use prelude::{search_articles, Article, ArticleQuery};

// service.rsから（ビジネスロジック）
pub use service::{
    fetch_and_store_article, fetch_and_store_article_with_client, get_article_content,
    get_article_content_with_client, search_article_contents, search_article_join_rows,
    search_article_url_statuses,
};

// repository.rsから（必要な低レベル操作）
pub use repository::store_article_content;

// types.rsから（型定義）
pub use model::{
    ArticleContent, ArticleContentQuery, ArticleJoinRow, ArticleJoinRowQuery, ArticleStatus,
    ArticleUrlStatus, ArticleUrlStatusQuery,
};

// query層は基本的に内部実装として隠蔽
// repository層も基本的に内部実装として隠蔽（必要に応じて公開可能）
