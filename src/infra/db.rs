use crate::types::{ConfigError, InfraError, InfraResult};
use sqlx::PgPool;
use std::env;

/// データベース接続プールを作成
/// .envファイルからDATABASE_URLを読み込みます
pub async fn create_pool() -> InfraResult<PgPool> {
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| InfraError::from(ConfigError::missing_env_var("DATABASE_URL")))?;
    
    PgPool::connect(&database_url)
        .await
        .map_err(|e| InfraError::database_connection(e))
}

/// データベースの初期化（マイグレーション実行）
pub async fn initialize_database(pool: &PgPool) -> InfraResult<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| InfraError::database_query("データベースマイグレーション実行", e.into()))
}

/// プールの作成とデータベース初期化を一括で行う便利関数
pub async fn setup_database() -> InfraResult<PgPool> {
    let pool = create_pool().await?;
    initialize_database(&pool).await?;
    Ok(pool)
}