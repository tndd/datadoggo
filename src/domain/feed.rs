use crate::infra::storage::file::load_yaml_from_file;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    pub group: String,
    pub name: String,
    pub rss_link: String,
}

impl fmt::Display for Feed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{} ({})", self.group, self.name, self.rss_link)
    }
}

// Feed検索のフィルター条件を表す構造体
#[derive(Debug, Default)]
pub struct FeedQuery {
    pub group: Option<String>,
    pub name: Option<String>,
}

impl FeedQuery {
    pub fn from_group(group: &str) -> Self {
        Self {
            group: Some(group.to_string()),
            name: None,
        }
    }
}

// YAMLファイルの構造に対応する型
type FeedMap = HashMap<String, HashMap<String, String>>;

/// フィード設定ファイルのパスを解決する
/// 優先順: CLI引数 > 環境変数(FEEDS_YAML) > 既定パス(data/feeds.yaml)
fn resolve_feeds_path(custom_path: Option<&str>) -> String {
    // 1. CLI引数が指定されている場合は最優先
    if let Some(path) = custom_path {
        println!("フィード設定: CLI引数で指定されたパス: {}", path);
        return path.to_string();
    }

    // 2. 環境変数をチェック
    if let Ok(env_path) = std::env::var("FEEDS_YAML") {
        if !env_path.is_empty() {
            println!("フィード設定: 環境変数FEEDS_YAMLで指定されたパス: {}", env_path);
            return env_path;
        }
    }

    // 3. 既定パス（まずは新しいパスを試す）
    let default_path = "data/feeds.yaml";
    if Path::new(default_path).exists() {
        println!("フィード設定: 既定パスを使用: {}", default_path);
        return default_path.to_string();
    }

    // 4. 後方互換性: 旧パスも試す
    let legacy_path = "src/domain/data/feeds.yaml";
    if Path::new(legacy_path).exists() {
        println!("⚠️ フィード設定: 旧パスを使用: {} ({}への移動を推奨)", legacy_path, default_path);
        return legacy_path.to_string();
    }

    // 5. どちらも存在しない場合は既定パスを返す（エラーは後続処理に任せる）
    println!("フィード設定: 既定パスを使用（ファイル未確認）: {}", default_path);
    default_path.to_string()
}

/// フィード情報を読み込み、Feedのベクタとして返す
fn load_feeds_from_yaml(file_path: &str) -> Result<Vec<Feed>> {
    let feed_map: FeedMap = load_yaml_from_file(file_path)
        .with_context(|| format!("フィードYAMLファイルの読み込みに失敗: {}", file_path))?;

    let mut feeds = Vec::new();

    for (group, name_links) in feed_map {
        for (name, link) in name_links {
            feeds.push(Feed {
                group: group.clone(),
                name,
                rss_link: link,
            });
        }
    }

    Ok(feeds)
}

/// フィード情報を3段階で絞り込み検索する
/// 1. 絞り込みなし（全件）
/// 2. groupのみ指定
/// 3. group & name指定
///
/// 内部でfeeds.yamlファイルを読み込み、指定されたクエリでフィルタリングする
/// カスタムパスが指定されない場合は、環境変数FEEDS_YAMLまたは既定パスを使用
pub fn search_feeds(query: Option<FeedQuery>) -> Result<Vec<Feed>> {
    search_feeds_with_path(query, None)
}

/// カスタムパスを指定してフィード情報を検索する
/// パス解決優先順: custom_path > 環境変数(FEEDS_YAML) > 既定パス(data/feeds.yaml)
pub fn search_feeds_with_path(query: Option<FeedQuery>, custom_path: Option<&str>) -> Result<Vec<Feed>> {
    let feeds_path = resolve_feeds_path(custom_path);
    let feeds = load_feeds_from_yaml(&feeds_path)?;
    let query = query.unwrap_or_default();

    let filtered_feeds = feeds
        .iter()
        .filter(|feed| {
            // groupフィルター
            if let Some(ref group_filter) = query.group {
                if feed.group != *group_filter {
                    return false;
                }
            }

            // nameフィルター（groupが指定されている場合のみ適用）
            if let Some(ref name_filter) = query.name {
                if feed.name != *name_filter {
                    return false;
                }
            }

            true
        })
        .cloned()
        .collect();

    Ok(filtered_feeds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_feeds_no_filter() {
        // 絞り込みなし（全件取得）
        let result = search_feeds(None);
        assert!(result.is_ok(), "フィード検索に失敗");

        let feeds = result.unwrap();
        assert!(!feeds.is_empty(), "フィードが取得されませんでした");
    }

    #[test]
    fn test_search_feeds_group_only() {
        // groupのみ絞り込み
        let query = FeedQuery {
            group: Some("bbc".to_string()),
            name: None,
        };
        let result = search_feeds(Some(query));
        assert!(result.is_ok(), "フィード検索に失敗");

        let feeds = result.unwrap();
        assert!(!feeds.is_empty(), "bbcグループのフィードが見つかりません");
        assert!(
            feeds.iter().all(|f| f.group == "bbc"),
            "全てbbcグループである必要があります"
        );
    }

    #[test]
    fn test_search_feeds_group_and_name() {
        // group & name絞り込み
        let query = FeedQuery {
            group: Some("bbc".to_string()),
            name: Some("world".to_string()),
        };
        let result = search_feeds(Some(query));
        assert!(result.is_ok(), "フィード検索に失敗");

        let feeds = result.unwrap();
        assert_eq!(feeds.len(), 1, "特定のフィードで1件が期待されます");
        assert_eq!(feeds[0].group, "bbc");
        assert_eq!(feeds[0].name, "world");
    }

    #[test]
    fn test_load_feeds_from_yaml() {
        // 実際のYAMLファイルからの読み込みテスト（search_feeds経由）
        let result = search_feeds(None);
        assert!(result.is_ok(), "YAMLファイルの読み込みに失敗");

        let feeds = result.unwrap();
        assert!(!feeds.is_empty(), "フィードが読み込まれませんでした");

        // bbcグループが存在することを確認
        let bbc_feeds: Vec<_> = feeds.iter().filter(|f| f.group == "bbc").collect();
        assert!(
            !bbc_feeds.is_empty(),
            "bbcグループのフィードが見つかりません"
        );

        println!(
            "✅ フィードYAML読み込みテスト成功: {}件のフィードを読み込み",
            feeds.len()
        );
    }

    #[test]
    fn test_feed_search_logic() {
        // フィード検索ロジックのテスト（外部通信なし）
        let query = FeedQuery {
            group: Some("存在しないグループ".to_string()),
            name: None,
        };

        let result = search_feeds(Some(query));
        match result {
            Ok(feeds) => {
                assert!(
                    feeds.is_empty(),
                    "存在しないグループでフィードが見つからないはず"
                );
            }
            Err(_) => {
                // ファイル読み込みエラーは許容
            }
        }

        println!("✅ フィード検索ロジックテスト完了");
    }

    #[test]
    fn test_search_feeds_with_path() {
        // カスタムパスを指定してフィード検索をテスト
        // 既存のdata/feeds.yamlを使用
        let result = search_feeds_with_path(None, Some("data/feeds.yaml"));
        assert!(result.is_ok(), "カスタムパス指定でのフィード検索に失敗");

        let feeds = result.unwrap();
        assert!(!feeds.is_empty(), "カスタムパスでフィードが取得されませんでした");

        // BBCグループの検索テスト
        let bbc_query = Some(FeedQuery {
            group: Some("bbc".to_string()),
            name: None,
        });
        let bbc_result = search_feeds_with_path(bbc_query, Some("data/feeds.yaml"));
        assert!(bbc_result.is_ok(), "BBC グループの検索に失敗");

        let bbc_feeds = bbc_result.unwrap();
        assert!(!bbc_feeds.is_empty(), "BBCフィードが見つかりません");
        assert!(
            bbc_feeds.iter().all(|f| f.group == "bbc"),
            "全てbbcグループである必要があります"
        );

        println!("✅ カスタムパス指定フィード検索テスト完了");
    }

    #[test]
    fn test_resolve_feeds_path() {
        // パス解決ロジックのテスト（環境変数なしの場合）
        
        // 環境変数が設定されていない場合、既定パスが返される
        std::env::remove_var("FEEDS_YAML");
        let path = resolve_feeds_path(None);
        
        // data/feeds.yamlが存在するはずなので、それが返されるべき
        assert_eq!(path, "data/feeds.yaml", "既定パスが返されませんでした");

        // カスタムパスが指定された場合、それが最優先される
        let custom_path = resolve_feeds_path(Some("custom/path.yaml"));
        assert_eq!(custom_path, "custom/path.yaml", "カスタムパスが正しく返されませんでした");

        println!("✅ パス解決ロジックテスト完了");
    }
}
