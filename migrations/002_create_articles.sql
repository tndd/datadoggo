-- urlは、rss_links.linkに依存するものとする
CREATE TABLE articles (
    url TEXT PRIMARY KEY,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    status_code INTEGER,
    content TEXT NOT NULL
);