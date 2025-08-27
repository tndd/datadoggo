pub mod db;
pub mod loader;

// 便利のため、よく使用される型を再エクスポート
pub use db::SaveResult;
pub use loader::FileLoader;