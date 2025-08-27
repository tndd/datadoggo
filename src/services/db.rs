use sqlx::{Error as SqlxError, PgPool};
use std::env;
use std::fmt;

/// データベースへの保存結果を格納する構造体
#[derive(Debug)]
pub struct SaveResult {
    pub inserted: usize,    // 新規にデータベースに挿入された記事
    pub skipped: usize,     // 重複によりスキップされた記事数
}

impl fmt::Display for SaveResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "処理完了: 新規保存{}件、重複スキップ{}件",
            self.inserted, self.skipped
        )
    }
}

/// データベース接続プールを作成
/// .envファイルからDATABASE_URLを読み込みます
pub async fn create_pool() -> Result<PgPool, SqlxError> {
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL environment variable must be set. Please check your .env file.");
    PgPool::connect(&database_url).await
}

/// データベースの初期化（マイグレーション実行）
pub async fn initialize_database(pool: &PgPool) -> Result<(), SqlxError> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(SqlxError::from)
}

/// プールの作成とデータベース初期化を一括で行う便利関数
pub async fn setup_database() -> Result<PgPool, SqlxError> {
    let pool = create_pool().await?;
    initialize_database(&pool).await?;
    Ok(pool)
}