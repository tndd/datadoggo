use thiserror::Error;

/// 設定関連のエラー型
/// 環境変数、設定ファイル、設定値の検証など設定に関するエラーを定義
#[derive(Error, Debug)]
pub enum ConfigError {
    /// 環境変数が見つからない
    #[error("環境変数が見つかりません: {name}")]
    MissingEnvironmentVariable { name: String },

}

impl ConfigError {
    /// 環境変数不足エラーを作成
    pub fn missing_env_var<N: Into<String>>(name: N) -> Self {
        Self::MissingEnvironmentVariable {
            name: name.into(),
        }
    }

}

