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

#[cfg(test)]
mod test_utils {
    use sqlx::PgPool;
    use std::time::Duration;
    use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};

    /// テスト用PostgreSQLコンテナの設定とプール接続を返す
    pub async fn setup_test_db() -> (
        testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
        PgPool,
    ) {
        let container = Postgres::default()
            .with_db_name("datadoggo_test")
            .with_user("test_user")
            .with_password("test_password")
            .start()
            .await
            .unwrap();

        let host_port = container.get_host_port_ipv4(5432).await.unwrap();

        let database_url = format!(
            "postgres://test_user:test_password@localhost:{}/datadoggo_test",
            host_port
        );

        // データベースが作成されるまで待機
        tokio::time::sleep(Duration::from_secs(3)).await;

        // マイグレーションを実行
        let temp_pool = PgPool::connect(&database_url).await.unwrap();
        sqlx::migrate!("./migrations")
            .run(&temp_pool)
            .await
            .unwrap();

        (container, temp_pool)
    }
}

#[cfg(test)]
pub use test_utils::setup_test_db;
