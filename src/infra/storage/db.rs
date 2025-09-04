use anyhow::{Context, Result};
use sqlx::PgPool;
use std::env;

/// データベース接続プールを作成
/// .envファイルからDATABASE_URLを読み込みます
pub async fn create_pool() -> Result<PgPool> {
    let database_url = env::var("DATABASE_URL")
        .context("データベースURLの環境変数DATABASE_URLが設定されていません")?;

    PgPool::connect(&database_url)
        .await
        .context("データベースへの接続に失敗しました")
}

/// データベースの初期化（マイグレーション実行）
pub async fn initialize_database(pool: &PgPool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("データベースマイグレーションの実行に失敗しました")
}

/// プールの作成とデータベース初期化を一括で行う便利関数
pub async fn setup_database() -> Result<PgPool> {
    let pool = create_pool().await?;
    initialize_database(&pool).await?;
    Ok(pool)
}
