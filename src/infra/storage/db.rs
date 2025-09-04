use anyhow::{Context, Result};
use sqlx::PgPool;
use std::env;

/// データベースインサート操作の結果を表す構造体
/// 新規挿入、重複時更新、スキップの件数を記録
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseInsertResult {
    /// 新規挿入された件数
    pub inserted: usize,
    /// 重複時に更新された件数
    pub updated: usize,
    /// 重複によりスキップされた件数（DO NOTHINGの場合）
    pub skipped: usize,
}

impl DatabaseInsertResult {
    /// 新しい操作結果を作成（旧互換性維持用）
    pub fn new(inserted: usize, skipped: usize) -> Self {
        Self {
            inserted,
            updated: 0,
            skipped,
        }
    }

    /// 完全な操作結果を作成
    pub fn new_complete(inserted: usize, updated: usize, skipped: usize) -> Self {
        Self {
            inserted,
            updated,
            skipped,
        }
    }

    /// 空の結果（全て0）を作成
    pub fn empty() -> Self {
        Self::new_complete(0, 0, 0)
    }

    /// ドメイン名を指定して表示用の文字列を生成
    pub fn display_with_domain(&self, domain_name: &str) -> String {
        if self.updated > 0 {
            format!(
                "{}処理完了: 新規{}件、更新{}件、重複スキップ{}件",
                domain_name, self.inserted, self.updated, self.skipped
            )
        } else {
            format!(
                "{}処理完了: 新規{}件、重複スキップ{}件",
                domain_name, self.inserted, self.skipped
            )
        }
    }
}

impl Default for DatabaseInsertResult {
    fn default() -> Self {
        Self::empty()
    }
}

// 汎用的なDisplay実装（デフォルトでは「データ」という名称を使用）
impl std::fmt::Display for DatabaseInsertResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_with_domain("データ"))
    }
}

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
