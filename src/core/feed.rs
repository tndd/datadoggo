use crate::infra::storage::file::load_yaml_from_file;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

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
pub fn search_feeds(query: Option<FeedQuery>) -> Result<Vec<Feed>> {
    let feeds = load_feeds_from_yaml("config/feeds.yaml")?;
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

    // 関数名ベースのモジュールへ統一
    mod search_feeds {
        use super::*;

        #[test]
        fn test_no_filter() {
            let result = super::super::search_feeds(None);
            assert!(result.is_ok());
            assert!(!result.unwrap().is_empty());
        }

        #[test]
        fn test_group_only() {
            let query = FeedQuery {
                group: Some("bbc".to_string()),
                name: None,
            };
            let feeds = super::super::search_feeds(Some(query)).unwrap();
            assert!(!feeds.is_empty());
            assert!(feeds.iter().all(|f| f.group == "bbc"));
        }

        #[test]
        fn test_group_and_name() {
            let query = FeedQuery {
                group: Some("bbc".to_string()),
                name: Some("world".to_string()),
            };
            let feeds = super::super::search_feeds(Some(query)).unwrap();
            assert_eq!(feeds.len(), 1);
            assert_eq!(feeds[0].group, "bbc");
            assert_eq!(feeds[0].name, "world");
        }

        #[test]
        fn test_load_feeds_from_yaml() {
            let feeds = super::super::search_feeds(None).unwrap();
            assert!(!feeds.is_empty());
            assert!(feeds.iter().any(|f| f.group == "bbc"));
        }
    }
}
