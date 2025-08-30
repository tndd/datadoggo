-- urlは、rss_links.linkに依存するものとする
CREATE TABLE articles (
    url TEXT PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT (now() AT TIME ZONE 'UTC'),
    status_code INTEGER NOT NULL,
    content TEXT NOT NULL
);