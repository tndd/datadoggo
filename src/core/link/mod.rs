mod command;
mod model;

pub use self::command::store_article_links;
pub use self::model::{ArticleLink, ArticleLinkQuery};

// 一時的な関数（後で適切なモジュールに移動予定）
use crate::core::feed::Feed;
use crate::infra::api::http::HttpClient;
use crate::infra::parser::{parse_channel_from_xml_str, parse_date};
use anyhow::{Context, Result};

use rss::Channel;

/// feedを用いてarticle_linkのリストを取得する
pub async fn fetch_article_links_using_feed<H: HttpClient>(
    client: &H,
    feed: &Feed,
) -> Result<Vec<ArticleLink>> {
    let xml_content = client
        .fetch(&feed.rss_link, 30)
        .await
        .context(format!("RSSフィードの取得に失敗: {}", feed))?;
    let channel = parse_channel_from_xml_str(&xml_content).context("XMLの解析に失敗")?;
    let article_links = get_article_links_from_channel(&channel);

    Ok(article_links)
}

// RSSのチャンネルから<item>要素のリンク情報を抽出する関数
fn get_article_links_from_channel(channel: &Channel) -> Vec<ArticleLink> {
    channel
        .items()
        .iter()
        .filter_map(|item| {
            let link = item.link()?;
            let pub_date_str = item.pub_date()?;
            let parsed_date = parse_date(pub_date_str).ok()?;

            Some(ArticleLink {
                url: link.to_string(),
                title: item.title().unwrap_or("タイトルなし").to_string(),
                pub_date: parsed_date,
                source: "rss".to_string(),
            })
        })
        .collect()
}
