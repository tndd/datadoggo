// フラット構造の下位モジュール
mod command;
mod model;
mod query;
mod service; // 内部ユーティリティ

// 外部から呼び出され得る関数/型はここで再エクスポートして集約（唯一の玄関口）
pub use self::command::{
    fetch_and_store_article, fetch_and_store_article_with_client, store_article_content,
};
pub use self::model::{
    Article, ArticleContent, ArticleJoinRow, ArticleJoinRowQuery, ArticleQuery, ArticleStatus,
    ArticleUrlStatus, ArticleUrlStatusQuery,
};
pub use self::query::{search_article_join_rows, search_article_url_statuses, search_articles};
pub use self::service::{fetch_article_content, fetch_article_content_with_client};
// article.rs はファサードのみ（実装・テストは下位モジュールへ配置）
