use thiserror::Error;

/// 設定関連のエラー型
/// 環境変数、設定ファイル、設定値の検証など設定に関するエラーを定義
#[derive(Error, Debug)]
pub enum ConfigError {
    /// 環境変数が見つからない
    #[error("環境変数が見つかりません: {name}")]
    MissingEnvironmentVariable { name: String },

    /// 設定値が不正
    #[error("設定値が不正です: {reason}")]
    InvalidValue { reason: String },

    /// 設定ファイルが見つからない
    #[error("設定ファイルが見つかりません: {path}")]
    MissingConfigFile { path: String },
}

impl ConfigError {
    /// 環境変数不足エラーを作成
    pub fn missing_env_var<N: Into<String>>(name: N) -> Self {
        Self::MissingEnvironmentVariable {
            name: name.into(),
        }
    }

    /// 不正な設定値エラーを作成
    pub fn invalid_value<R: Into<String>>(reason: R) -> Self {
        Self::InvalidValue {
            reason: reason.into(),
        }
    }

    /// 設定ファイル不足エラーを作成
    pub fn missing_config_file<P: Into<String>>(path: P) -> Self {
        Self::MissingConfigFile {
            path: path.into(),
        }
    }
}

/// 設定エラーのResult型エイリアス
pub type ConfigResult<T> = std::result::Result<T, ConfigError>;