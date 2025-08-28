//! 型定義モジュール
//! 
//! アプリケーション全体で使用される共通的な型定義を管理します。
//! - 共通エラー型: 複数のモジュールで使用される基盤的なエラー
//! - 結果型: 操作結果の表現

pub mod error;
pub mod result;

// 便利な再エクスポート
pub use error::{CommonError, CommonResult};
pub use result::SaveResult;