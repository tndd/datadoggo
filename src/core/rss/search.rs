use super::model::{RssLink, RssLinkMap, RssLinkQuery};
use crate::infra::storage::file::load_yaml_from_file;
use anyhow::{Context, Result};

/// フィード情報を3段階で絞り込み検索する
/// 1. 絞り込みなし（全件）
/// 2. groupのみ指定
/// 3. group & name指定
///
/// 内部でcore/rss/link.ymlファイルを読み込み、指定されたクエリでフィルタリングする
pub fn search_feeds(query: Option<RssLinkQuery>) -> Result<Vec<RssLink>> {
    let feeds = load_feeds_from_yaml("src/core/rss/link.yml")?;
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

/// core/rss/link.ymlからフィード情報を読み込み、RssLinkのベクタとして返す
fn load_feeds_from_yaml(file_path: &str) -> Result<Vec<RssLink>> {
    let rss_link_map: RssLinkMap = load_yaml_from_file(file_path)
        .with_context(|| format!("RSSリンクYAMLファイルの読み込みに失敗: {}", file_path))?;

    let mut feeds = Vec::new();

    for (group, name_links) in rss_link_map {
        for (name, link) in name_links {
            feeds.push(RssLink {
                group: group.clone(),
                name,
                url: link,
            });
        }
    }

    Ok(feeds)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod search_feeds {
        use super::*;

        #[test]
        fn test_no_filter() {
            let result = search_feeds(None);
            assert!(result.is_ok());
            assert!(!result.unwrap().is_empty());
        }

        #[test]
        fn test_group_only() {
            let query = RssLinkQuery {
                group: Some("bbc".to_string()),
                name: None,
            };
            let feeds = search_feeds(Some(query)).unwrap();
            assert!(!feeds.is_empty());
            assert!(feeds.iter().all(|f| f.group == "bbc"));
        }

        #[test]
        fn test_group_and_name() {
            let query = RssLinkQuery {
                group: Some("bbc".to_string()),
                name: Some("world".to_string()),
            };
            let feeds = search_feeds(Some(query)).unwrap();
            assert_eq!(feeds.len(), 1);
            assert_eq!(feeds[0].group, "bbc");
            assert_eq!(feeds[0].name, "world");
        }
    }

    mod load_feeds_from_yaml {
        use super::*;

        #[test]
        fn test_load_feeds_from_yaml() {
            let feeds = search_feeds(None).unwrap();
            assert!(!feeds.is_empty());
            assert!(feeds.iter().any(|f| f.group == "bbc"));
        }
    }
}
