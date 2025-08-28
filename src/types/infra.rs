//! インフラストラクチャ型定義
//! 
//! データベース操作の結果型など、インフラ層で使用される型を定義

/// データベースインサート操作の結果を表す構造体
/// 新規挿入、重複スキップの件数を記録
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseInsertResult {
    /// 新規挿入された件数
    pub inserted: usize,
    /// 重複によりスキップされた件数
    pub skipped_duplicate: usize,
}

impl DatabaseInsertResult {
    /// 新しい操作結果を作成
    pub fn new(inserted: usize, skipped: usize) -> Self {
        Self {
            inserted,
            skipped_duplicate: skipped,
        }
    }

    /// 空の結果（全て0）を作成
    pub fn empty() -> Self {
        Self::new(0, 0)
    }

    /// ドメイン名を指定して表示用の文字列を生成
    pub fn display_with_domain(&self, domain_name: &str) -> String {
        format!(
            "{}処理完了: 新規{}件、重複スキップ{}件",
            domain_name, self.inserted, self.skipped_duplicate
        )
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