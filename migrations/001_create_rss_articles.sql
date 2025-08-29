CREATE TABLE IF NOT EXISTS rss_articles (
    link        TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    description TEXT,
    pub_date    TEXT
);