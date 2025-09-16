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
pub type FeedMap = HashMap<String, HashMap<String, String>>;

#[cfg(test)]
mod tests {
    use super::*;

    mod feed_display {
        use super::*;

        #[test]
        fn test_feed_display_format() {
            let feed = Feed {
                group: "bbc".to_string(),
                name: "world".to_string(),
                rss_link: "https://feeds.bbci.co.uk/news/world/rss.xml".to_string(),
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
            let query = FeedQuery::from_group("bbc");
            assert_eq!(query.group, Some("bbc".to_string()));
            assert_eq!(query.name, None);
        }

        #[test]
        fn test_default() {
            let query = FeedQuery::default();
            assert_eq!(query.group, None);
            assert_eq!(query.name, None);
        }
    }
}
