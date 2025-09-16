use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssLink {
    pub group: String,
    pub name: String,
    pub url: String,
}

// TODO: 正式なログ実装
impl fmt::Display for RssLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{} ({})", self.group, self.name, self.url)
    }
}

// RSSリンク検索のフィルター条件を表す構造体
#[derive(Debug, Default)]
pub struct RssLinkQuery {
    pub group: Option<String>,
    pub name: Option<String>,
}

impl RssLinkQuery {
    pub fn from_group(group: &str) -> Self {
        Self {
            group: Some(group.to_string()),
            name: None,
        }
    }
}

// YAMLファイルの構造に対応する型
pub type RssLinkMap = HashMap<String, HashMap<String, String>>;

#[cfg(test)]
mod tests {
    use super::*;

    mod feed_display {
        use super::*;

        #[test]
        fn test_feed_display_format() {
            let feed = RssLink {
                group: "bbc".to_string(),
                name: "world".to_string(),
                url: "https://feeds.bbci.co.uk/news/world/rss.xml".to_string(),
            };
            assert_eq!(
                format!("{}", feed),
                "bbc/world (https://feeds.bbci.co.uk/news/world/rss.xml)"
            );
        }
    }

    mod feed_query {
        use super::*;

        #[test]
        fn test_from_group() {
            let query = RssLinkQuery::from_group("bbc");
            assert_eq!(query.group, Some("bbc".to_string()));
            assert_eq!(query.name, None);
        }

        #[test]
        fn test_default() {
            let query = RssLinkQuery::default();
            assert_eq!(query.group, None);
            assert_eq!(query.name, None);
        }
    }
}
