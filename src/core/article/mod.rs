pub mod model;
pub mod prelude;
pub mod repository;
pub mod service;

// prelude.rsから（ユーザー向けAPI）
pub use prelude::{search_articles, Article, ArticleQuery};

// service.rsから（ビジネスロジック）
// 微妙な関数（外部API呼び出しや複合処理）のみを公開
pub use service::{
    fetch_and_store_article, fetch_and_store_article_with_client, get_article_content,
    get_article_content_with_client,
};

// repository.rsから（データ取得系の公開API）
pub use repository::{
    search_article_contents, search_article_join_rows, search_article_url_statuses,
    store_article_content,
};

// types.rsから（型定義）
pub use model::{
    ArticleContent, ArticleContentQuery, ArticleJoinRow, ArticleJoinRowQuery, ArticleStatus,
    ArticleUrlStatus, ArticleUrlStatusQuery,
};

// query層は基本的に内部実装として隠蔽
// repository層も基本的に内部実装として隠蔽（必要に応じて公開可能）
