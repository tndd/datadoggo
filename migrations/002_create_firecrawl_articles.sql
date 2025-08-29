-- urlは、rss_articles.linkに依存するものとする
CREATE TABLE IF NOT EXISTS firecrawl_articles (
    url TEXT PRIMARY KEY,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    status_code INTEGER,
    markdown TEXT NOT NULL
);