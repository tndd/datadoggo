//! 型定義モジュール
//! 
//! アプリケーション全体で使用される共通的な型定義を管理します。
//! - データベース操作結果型: インサート結果の統一表現

pub mod infra;

// 便利な再エクスポート  
pub use infra::DatabaseInsertResult;