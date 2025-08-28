use anyhow::{Context, Result};
use std::fs::File;
use std::io::BufReader;

/// ファイルパスからBufReaderを作成する
/// パースやデータ変換は各ドメインで行う
pub fn load_file(file_path: &str) -> Result<BufReader<File>> {
    let file = File::open(file_path)
        .with_context(|| format!("ファイルの読み込みに失敗しました: {}", file_path))?;
    let buf_reader = BufReader::new(file);
    Ok(buf_reader)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_existing_file() {
        // 存在するファイルを読み込めることを確認
        let result = load_file("mock/fc/bbc.json");
        assert!(result.is_ok(), "既存ファイルの読み込みに失敗");
    }

    #[test]
    fn test_load_non_existing_file() {
        // 存在しないファイルでエラーになることを確認
        let result = load_file("non_existent_file.txt");
        assert!(result.is_err(), "存在しないファイルでエラーにならなかった");
    }
}