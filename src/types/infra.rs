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

/// データベースインサート操作の結果を表す構造体
/// 新規挿入、重複スキップ、更新の件数を記録
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseInsertResult {
    /// 新規挿入された件数
    pub inserted: usize,
    /// 重複によりスキップされた件数
    pub skipped_duplicate: usize,
    /// 更新された件数
    pub updated: usize,
}

impl DatabaseInsertResult {
    /// 新しい操作結果を作成
    pub fn new(inserted: usize, skipped: usize, updated: usize) -> Self {
        Self {
            inserted,
            skipped_duplicate: skipped,
            updated,
        }
    }

    /// 空の結果（全て0）を作成
    pub fn empty() -> Self {
        Self::new(0, 0, 0)
    }


    /// ドメイン名を指定して表示用の文字列を生成
    pub fn display_with_domain(&self, domain_name: &str) -> String {
        format!(
            "{}処理完了: 新規{}件、重複スキップ{}件、更新{}件",
            domain_name, self.inserted, self.skipped_duplicate, self.updated
        )
    }
}

impl Default for DatabaseInsertResult {
    fn default() -> Self {
        Self::empty()
    }
}

// 汎用的なDisplay実装（デフォルトでは「データ」という名称を使用）
impl std::fmt::Display for DatabaseInsertResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_with_domain("データ"))
    }
}