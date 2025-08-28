//! 型定義モジュール
//! 
//! アプリケーション全体で使用される共通的な型定義を管理します。
//! - インフラエラー型: データベース、ファイルシステム等の基盤エラー
//! - 設定エラー型: 環境変数、設定値等の設定関連エラー

pub mod infra;
pub mod config;

// 便利な再エクスポート
pub use infra::{InfraError, InfraResult};
pub use config::ConfigError;