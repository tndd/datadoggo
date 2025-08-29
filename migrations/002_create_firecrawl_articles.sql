CREATE TABLE IF NOT EXISTS firecrawl_articles (
    id              SERIAL PRIMARY KEY,
    url             TEXT UNIQUE NOT NULL,
    title           TEXT,
    markdown_content TEXT NOT NULL,
    metadata_json   JSONB NOT NULL,
    scraped_at      TIMESTAMP,
    created_at      TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at      TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- URL用のインデックス（高速検索のため）
CREATE INDEX IF NOT EXISTS idx_firecrawl_articles_url ON firecrawl_articles(url);

-- タイトル用のインデックス（検索用）
CREATE INDEX IF NOT EXISTS idx_firecrawl_articles_title ON firecrawl_articles(title);

-- 作成日時用のインデックス（時系列ソート用）
CREATE INDEX IF NOT EXISTS idx_firecrawl_articles_created_at ON firecrawl_articles(created_at);

-- メタデータのJSON検索用インデックス（特定のフィールド検索を高速化）
CREATE INDEX IF NOT EXISTS idx_firecrawl_articles_metadata_source_url ON firecrawl_articles((metadata_json->>'sourceURL'));