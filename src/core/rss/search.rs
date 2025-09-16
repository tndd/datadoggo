use super::model::{RssLink, RssLinkMap, RssLinkQuery};
use crate::infra::storage::file::load_yaml_from_file;
use anyhow::{Context, Result};

/// フィード情報を3段階で絞り込み検索する
/// 1. 絞り込みなし（全件）
/// 2. groupのみ指定
/// 3. group & name指定
///
/// 内部でcore/rss/link.ymlファイルを読み込み、指定されたクエリでフィルタリングする
pub fn search_rss_links(query: Option<&RssLinkQuery>) -> Result<Vec<RssLink>> {
    let rss_links = load_rss_links_from_yaml("src/core/rss/link.yml")?;
    let query = query.cloned().unwrap_or_default();

    let filtered_rss_links = rss_links
        .iter()
        .filter(|rss_link| {
            // groupフィルター
            if let Some(ref group_filter) = query.group {
                if rss_link.group != *group_filter {
                    return false;
                }
            }

            // nameフィルター（groupが指定されている場合のみ適用）
            if let Some(ref name_filter) = query.name {
                if rss_link.name != *name_filter {
                    return false;
                }
            }

            true
        })
        .cloned()
        .collect();

    Ok(filtered_rss_links)
}

/// core/rss/link.ymlからフィード情報を読み込み、RssLinkのベクタとして返す
fn load_rss_links_from_yaml(file_path: &str) -> Result<Vec<RssLink>> {
    let yaml_value = load_yaml_from_file(file_path)
        .with_context(|| format!("RSSリンクYAMLファイルの読み込みに失敗: {}", file_path))?;

    let rss_link_map: RssLinkMap = serde_yaml::from_value(yaml_value)
        .with_context(|| format!("YAMLデータの変換に失敗: {}", file_path))?;

    let mut rss_links = Vec::new();

    for (group, name_links) in rss_link_map {
        for (name, link) in name_links {
            rss_links.push(RssLink {
                group: group.clone(),
                name,
                url: link,
            });
        }
    }

    Ok(rss_links)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod search_rss_links {
        use super::*;

        #[test]
        fn test_no_filter() {
            let result = search_rss_links(None);
            assert!(result.is_ok());
            assert!(!result.unwrap().is_empty());
        }

        #[test]
        fn test_group_only() {
            let query = RssLinkQuery {
                group: Some("bbc".to_string()),
                name: None,
            };
            let rss_links = search_rss_links(Some(&query)).unwrap();
            assert!(!rss_links.is_empty());
            assert!(rss_links.iter().all(|f| f.group == "bbc"));
        }

        #[test]
        fn test_group_and_name() {
            let query = RssLinkQuery {
                group: Some("bbc".to_string()),
                name: Some("world".to_string()),
            };
            let rss_links = search_rss_links(Some(&query)).unwrap();
            assert_eq!(rss_links.len(), 1);
            assert_eq!(rss_links[0].group, "bbc");
            assert_eq!(rss_links[0].name, "world");
        }
    }

    mod load_rss_links_from_yaml {
        use super::*;

        #[test]
        fn test_load_rss_links_from_yaml() {
            let rss_links = load_rss_links_from_yaml("src/core/rss/link.yml").unwrap();
            assert!(!rss_links.is_empty());
            assert!(rss_links.iter().any(|f| f.group == "bbc"));
        }
    }
}
