-- urlは、rss_links.linkに依存するものとする
CREATE TABLE articles (
    url TEXT PRIMARY KEY,
    timestamp TIMESTAMPTZ DEFAULT (now() AT TIME ZONE 'UTC'),
    status_code INTEGER,
    content TEXT NOT NULL
);