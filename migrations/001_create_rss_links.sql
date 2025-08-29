-- RSSリンク用テーブル（RssLink構造に対応）
CREATE TABLE rss_links (
    link TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    pub_date TEXT NOT NULL
);