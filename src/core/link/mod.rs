mod model;

pub use self::model::{ArticleLink, ArticleLinkQuery};

// 一時的な関数（後で適切なモジュールに移動予定）
use crate::core::feed::Feed;
use crate::infra::api::http::HttpClient;
use crate::infra::parser::{parse_channel_from_xml_str, parse_date};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rss::Channel;
use sqlx::PgPool;

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

/// # 概要
/// ArticleLinkの配列を指定されたデータベースプールに保存する。
pub async fn store_article_links(article_links: &[ArticleLink], pool: &PgPool) -> Result<()> {
    if article_links.is_empty() {
        return Ok(());
    }

    // 配列として渡すためのデータ準備
    let urls: Vec<String> = article_links.iter().map(|r| r.url.clone()).collect();
    let titles: Vec<String> = article_links.iter().map(|r| r.title.clone()).collect();
    let pub_dates: Vec<DateTime<Utc>> = article_links.iter().map(|r| r.pub_date).collect();
    let sources: Vec<String> = article_links.iter().map(|r| r.source.clone()).collect();

    // バルクUPSERT処理
    sqlx::query!(
        r#"
        INSERT INTO article_links (url, title, pub_date, source)
        SELECT * FROM UNNEST($1::text[], $2::text[], $3::timestamptz[], $4::text[])
        ON CONFLICT (url) DO UPDATE SET
            title = EXCLUDED.title,
            pub_date = EXCLUDED.pub_date,
            source = EXCLUDED.source
        WHERE (article_links.title, article_links.pub_date, article_links.source)
            IS DISTINCT FROM (EXCLUDED.title, EXCLUDED.pub_date, EXCLUDED.source)
        "#,
        &urls,
        &titles,
        &pub_dates,
        &sources
    )
    .execute(pool)
    .await
    .context("記事リンクのバルクUPSERT処理に失敗しました")?;

    Ok(())
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
