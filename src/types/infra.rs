use thiserror::Error;
use crate::types::ConfigError;

/// インフラストラクチャ層のエラー型
/// データベース、ファイルシステム、シリアライゼーションなど基盤的なエラーを定義
#[derive(Error, Debug)]
pub enum InfraError {
    /// ファイルシステムエラー
    #[error("ファイルシステムエラー: {path} - {source}")]
    FileSystem {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// データベース接続エラー
    #[error("データベース接続エラー: {source}")]
    DatabaseConnection {
        #[source]
        source: sqlx::Error,
    },

    /// データベースクエリエラー
    #[error("データベースクエリエラー: {operation} - {source}")]
    DatabaseQuery {
        operation: String,
        #[source]
        source: sqlx::Error,
    },

    /// シリアライゼーションエラー
    #[error("シリアライゼーションエラー: {context} - {source}")]
    Serialization {
        context: String,
        #[source]
        source: serde_json::Error,
    },

    /// 設定エラー
    #[error(transparent)]
    Config(#[from] ConfigError),
}

impl InfraError {
    /// ファイルシステムエラーを作成
    pub fn file_system<P: Into<String>>(path: P, source: std::io::Error) -> Self {
        Self::FileSystem {
            path: path.into(),
            source,
        }
    }

    /// データベース接続エラーを作成
    pub fn database_connection(source: sqlx::Error) -> Self {
        Self::DatabaseConnection { source }
    }

    /// データベースクエリエラーを作成
    pub fn database_query<O: Into<String>>(operation: O, source: sqlx::Error) -> Self {
        Self::DatabaseQuery {
            operation: operation.into(),
            source,
        }
    }

    /// シリアライゼーションエラーを作成
    pub fn serialization<C: Into<String>>(context: C, source: serde_json::Error) -> Self {
        Self::Serialization {
            context: context.into(),
            source,
        }
    }
}

/// インフラエラーのResult型エイリアス
pub type InfraResult<T> = std::result::Result<T, InfraError>;