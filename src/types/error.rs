use thiserror::Error;

/// アプリケーション共通のエラー型
/// 複数のモジュールで使用される基盤的なエラーのみを定義
#[derive(Error, Debug)]
pub enum CommonError {
    /// ファイルI/Oエラー
    #[error("ファイル操作エラー: {path} - {source}")]
    FileIo {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// データベース関連のエラー
    #[error("データベースエラー: {operation} - {source}")]
    Database {
        operation: String,
        #[source]
        source: sqlx::Error,
    },

    /// JSONシリアライゼーション/デシリアライゼーションエラー
    #[error("JSON処理エラー: {context} - {source}")]
    Json {
        context: String,
        #[source]
        source: serde_json::Error,
    },

    /// 設定関連のエラー
    #[error("設定エラー: {message}")]
    Config { message: String },
}

impl CommonError {
    /// ファイルI/Oエラーを作成
    pub fn file_io<P: Into<String>>(path: P, source: std::io::Error) -> Self {
        Self::FileIo {
            path: path.into(),
            source,
        }
    }

    /// データベースエラーを作成
    pub fn database<O: Into<String>>(operation: O, source: sqlx::Error) -> Self {
        Self::Database {
            operation: operation.into(),
            source,
        }
    }

    /// JSON処理エラーを作成
    pub fn json<C: Into<String>>(context: C, source: serde_json::Error) -> Self {
        Self::Json {
            context: context.into(),
            source,
        }
    }

    /// 設定エラーを作成
    pub fn config<M: Into<String>>(message: M) -> Self {
        Self::Config {
            message: message.into(),
        }
    }
}

/// 共通エラーのResult型エイリアス
pub type CommonResult<T> = std::result::Result<T, CommonError>;