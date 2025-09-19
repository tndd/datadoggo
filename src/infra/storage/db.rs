use anyhow::{Context, Result};
use sqlx::PgPool;
use std::env;

/// データベース接続プールを作成
/// 環境変数に基づいて適切なDATABASE_URLを選択します
/// ENVIRONMENT=prod の場合は本番用DB、デフォルトはテスト/開発用DBを使用
pub async fn create_pool() -> Result<PgPool> {
    let database_url = get_database_url()?;

    PgPool::connect(&database_url)
        .await
        .context("データベースへの接続に失敗しました")
}

/// 環境変数に基づいて適切なDATABASE_URLを取得
fn get_database_url() -> Result<String> {
    // 環境変数ENVIRONMENTをチェック
    let environment = env::var("ENVIRONMENT").unwrap_or_default();

    let url = match environment.as_str() {
        "prod" => {
            // 本番環境: DATABASE_URL_PRODのみを使用、フォールバックなし
            env::var("DATABASE_URL_PROD")
                .context("DATABASE_URL_PROD is not configured for production environment")?
        }
        _ => {
            // デフォルト（テスト・開発）: DATABASE_URLを使用
            env::var("DATABASE_URL").context("DATABASE_URL is not configured")?
        }
    };

    Ok(url)
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
