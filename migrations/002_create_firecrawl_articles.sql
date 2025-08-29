-- firecrawl_articlesテーブルの作成
-- urlをプライマリキーとし、rss_articles.linkと等価な状態

CREATE TABLE IF NOT EXISTS firecrawl_articles (
    url TEXT PRIMARY KEY,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    status_code INTEGER,
    markdown TEXT NOT NULL
);

-- インデックスを作成（created_at用のみ）
CREATE INDEX IF NOT EXISTS idx_firecrawl_articles_created_at ON firecrawl_articles(created_at);