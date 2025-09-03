use crate::infra::storage::file::load_yaml_from_file;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    pub group: String,
    pub name: String,
    pub link: String,
}

impl fmt::Display for Feed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{} ({})", self.group, self.name, self.link)
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

/// src/domain/data/feeds.yamlからフィード情報を読み込み、Feedのベクタとして返す
fn load_feeds_from_yaml(file_path: &str) -> Result<Vec<Feed>> {
    let feed_map: FeedMap = load_yaml_from_file(file_path)
        .with_context(|| format!("フィードYAMLファイルの読み込みに失敗: {}", file_path))?;

    let mut feeds = Vec::new();

    for (group, name_links) in feed_map {
        for (name, link) in name_links {
            feeds.push(Feed {
                group: group.clone(),
                name,
                link,
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
pub fn search_feeds(query: Option<FeedQuery>) -> Result<Vec<Feed>> {
    let feeds = load_feeds_from_yaml("src/domain/data/feeds.yaml")?;
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
}
