/// 保存結果の詳細情報
#[derive(Debug, Clone)]
pub struct SaveResult {
    pub inserted: usize,
    pub skipped: usize,
    pub updated: usize,
}

impl std::fmt::Display for SaveResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "処理完了: 新規保存{}件、スキップ{}件、更新{}件",
            self.inserted, self.skipped, self.updated
        )
    }
}

impl SaveResult {
    /// 新しい保存結果を作成
    pub fn new(inserted: usize, skipped: usize, updated: usize) -> Self {
        Self {
            inserted,
            skipped,
            updated,
        }
    }

    /// 空の結果を作成
    pub fn empty() -> Self {
        Self::new(0, 0, 0)
    }

}