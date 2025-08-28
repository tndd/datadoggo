use crate::types::{CommonError, CommonResult};
use sqlx::PgPool;
use std::env;

/// データベース接続プールを作成
/// .envファイルからDATABASE_URLを読み込みます
pub async fn create_pool() -> CommonResult<PgPool> {
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| CommonError::config("DATABASE_URL環境変数が設定されていません。.envファイルを確認してください。"))?;
    
    PgPool::connect(&database_url)
        .await
        .map_err(|e| CommonError::database("データベース接続プール作成", e))
}

/// データベースの初期化（マイグレーション実行）
pub async fn initialize_database(pool: &PgPool) -> CommonResult<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| CommonError::database("データベースマイグレーション実行", e.into()))
}

/// プールの作成とデータベース初期化を一括で行う便利関数
pub async fn setup_database() -> CommonResult<PgPool> {
    let pool = create_pool().await?;
    initialize_database(&pool).await?;
    Ok(pool)
}