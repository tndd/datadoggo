pub mod model;
pub mod search;

// 公開型の再エクスポート
pub use model::{RssLink, RssLinkQuery};
pub use search::search_feeds;
