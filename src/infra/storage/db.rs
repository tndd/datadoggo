use anyhow::{Context, Result};
use sqlx::PgPool;
use std::env;

/// データベース接続プールを作成
/// 環境変数に基づいて適切なDATABASE_URLを選択します
/// ENVIRONMENT=prod の場合は本番/開発用DB、デフォルトはテスト用DBを使用
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
            // 本番/開発環境: PROD_DB_URLを使用、フォールバックあり
            env::var("PROD_DB_URL")
                .or_else(|_| env::var("DATABASE_URL"))
                .context("ENVIRONMENT is set to 'prod' but PROD_DB_URL is not configured")?
        }
        "test" | "" => {
            // テスト環境: TEST_DB_URLを使用、フォールバックあり
            env::var("TEST_DB_URL")
                .or_else(|_| env::var("DATABASE_URL"))
                .context("ENVIRONMENT is set to 'test' but TEST_DB_URL is not configured")?
        }
        _ => {
            anyhow::bail!(
                "Invalid ENVIRONMENT value: '{}'. Must be 'prod', 'test', or empty",
                environment
            );
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
