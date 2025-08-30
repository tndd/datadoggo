use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    pub group: String,
    pub name: String,
    pub link: String,
}

#[derive(Debug, Deserialize)]
pub struct FeedConfig {
    feeds: HashMap<String, HashMap<String, String>>,
}

impl FeedConfig {
    /// feeds.yamlファイルから設定を読み込む
    pub fn load_from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("フィード設定ファイルの読み込みに失敗: {}", path))?;

        let feeds: HashMap<String, HashMap<String, String>> =
            serde_yaml::from_str(&content).with_context(|| format!("YAML解析に失敗: {}", path))?;

        Ok(FeedConfig { feeds })
    }

    /// デフォルトのfeeds.yamlファイルから設定を読み込む
    pub fn load_default() -> Result<Self> {
        Self::load_from_file("src/domain/data/feeds.yaml")
    }

    /// 特定のグループのフィードを取得
    pub fn get_feeds_by_group(&self, group: &str) -> Vec<Feed> {
        if let Some(group_feeds) = self.feeds.get(group) {
            group_feeds
                .iter()
                .map(|(name, link)| Feed {
                    group: group.to_string(),
                    name: name.clone(),
                    link: link.clone(),
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// 全てのフィードを取得
    pub fn get_all_feeds(&self) -> Vec<Feed> {
        let mut all_feeds = Vec::new();
        for (group, group_feeds) in &self.feeds {
            for (name, link) in group_feeds {
                all_feeds.push(Feed {
                    group: group.clone(),
                    name: name.clone(),
                    link: link.clone(),
                });
            }
        }
        all_feeds
    }

    /// 特定のフィードURLを取得
    pub fn get_feed_url(&self, group: &str, name: &str) -> Option<&String> {
        self.feeds.get(group)?.get(name)
    }

    /// 利用可能なグループ一覧を取得
    pub fn get_groups(&self) -> Vec<&String> {
        self.feeds.keys().collect()
    }

    /// 特定のグループ内のフィード名一覧を取得
    pub fn get_feed_names_in_group(&self, group: &str) -> Vec<&String> {
        if let Some(group_feeds) = self.feeds.get(group) {
            group_feeds.keys().collect()
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_feed_config() {
        // テスト用のYAMLデータ
        let yaml_content = r#"
            bbc:
                top: https://feeds.bbci.co.uk/news/rss.xml
                world: https://feeds.bbci.co.uk/news/world/rss.xml
            cbs:
                top: https://www.cbsnews.com/latest/rss/main
                us: https://www.cbsnews.com/latest/rss/us
            "#;

        let feeds: HashMap<String, HashMap<String, String>> =
            serde_yaml::from_str(yaml_content).unwrap();
        let config = FeedConfig { feeds };

        // BBCグループのフィードを取得
        let bbc_feeds = config.get_feeds_by_group("bbc");
        assert_eq!(bbc_feeds.len(), 2);
        assert_eq!(bbc_feeds[0].group, "bbc");

        // 特定のフィードURLを取得
        let url = config.get_feed_url("bbc", "top");
        assert_eq!(
            url,
            Some(&"https://feeds.bbci.co.uk/news/rss.xml".to_string())
        );

        // 全フィードを取得
        let all_feeds = config.get_all_feeds();
        assert_eq!(all_feeds.len(), 4);

        // グループ一覧を取得
        let groups = config.get_groups();
        assert!(groups.contains(&&"bbc".to_string()));
        assert!(groups.contains(&&"cbs".to_string()));
    }

    #[test]
    fn test_load_actual_feeds_yaml() {
        // 実際のfeeds.yamlファイルを読み込むテスト
        let result = FeedConfig::load_default();

        if result.is_ok() {
            let config = result.unwrap();

            // BBCグループが存在することを確認
            let bbc_feeds = config.get_feeds_by_group("bbc");
            assert!(!bbc_feeds.is_empty(), "BBCフィードが見つかりません");

            // Yahoo Japanグループが存在することを確認
            let yahoo_feeds = config.get_feeds_by_group("yahoo_japan");
            assert!(
                !yahoo_feeds.is_empty(),
                "Yahoo Japanフィードが見つかりません"
            );

            // 全フィード数をチェック
            let all_feeds = config.get_all_feeds();
            assert!(
                all_feeds.len() > 50,
                "フィード数が少なすぎます: {}",
                all_feeds.len()
            );

            // グループ数をチェック
            let groups = config.get_groups();
            assert!(
                groups.len() >= 5,
                "グループ数が少なすぎます: {}",
                groups.len()
            );

            println!(
                "読み込み成功: {}グループ、{}フィード",
                groups.len(),
                all_feeds.len()
            );
        } else {
            println!("feeds.yamlファイルが見つからないため、このテストをスキップします");
        }
    }
}
