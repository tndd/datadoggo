-- RSSリンク用テーブル（RssLink構造に対応）
CREATE TABLE rss_articles (
    link     TEXT PRIMARY KEY,
    title    TEXT NOT NULL,
    pub_date TEXT NOT NULL
);