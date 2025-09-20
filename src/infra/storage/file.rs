use crate::infra::parser::parse_channel_from_reader;
use anyhow::{Context, Result};
use rss::Channel;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

#[track_caller]
fn resolve_path(file_path: &str) -> PathBuf {
    let caller = std::panic::Location::caller();

    if file_path.starts_with('.') {
        let caller_path = Path::new(caller.file());
        let base_dir = caller_path.parent().unwrap_or_else(|| Path::new("."));
        base_dir.join(file_path)
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(file_path)
    }
}

/// ファイルパスからBufReaderを作成する
/// パースやデータ変換は各ドメインで行う
#[track_caller]
pub fn load_file(file_path: &str) -> Result<BufReader<File>> {
    let resolved_path = resolve_path(file_path);
    let file = File::open(&resolved_path).with_context(|| {
        format!(
            "ファイルの読み込みに失敗しました: {}",
            resolved_path.display()
        )
    })?;
    let buf_reader = BufReader::new(file);
    Ok(buf_reader)
}

/// xmlファイルからchannelを読み込む
#[track_caller]
pub fn load_channel_from_xml_file(file_path: &str) -> Result<Channel> {
    let buf_reader = load_file(file_path)?;
    parse_channel_from_reader(buf_reader)
        .with_context(|| format!("RSSファイルの解析に失敗: {}", file_path))
}

/// JSONファイルからserde_json::Valueを読み込む
#[track_caller]
pub fn load_json_from_file(file_path: &str) -> Result<serde_json::Value> {
    let buf_reader = load_file(file_path)?;
    serde_json::from_reader(buf_reader)
        .with_context(|| format!("JSONファイルの解析に失敗: {}", file_path))
}

/// YAMLファイルからserde_yaml::Valueを読み込む
#[track_caller]
pub fn load_yaml_from_file(file_path: &str) -> Result<serde_yaml::Value> {
    let buf_reader = load_file(file_path)?;
    serde_yaml::from_reader(buf_reader)
        .with_context(|| format!("YAMLファイルの解析に失敗: {}", file_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ヘルパー群はhelperモジュールにまとめる
    mod helper {
        use super::*;

        #[test]
        fn test_load_existing_file() {
            let result = load_file("./mock/sample.json");
            assert!(result.is_ok());
        }

        #[test]
        fn test_load_existing_file_relative() {
            let result = load_file("./mock/sample.json");
            assert!(result.is_ok());
        }

        #[test]
        fn test_load_non_existing_file() {
            let result = load_file("./mock/non_existent_file.txt");
            assert!(result.is_err());
        }
    }
}
