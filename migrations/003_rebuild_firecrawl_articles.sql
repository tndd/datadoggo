-- firecrawl_articlesテーブルを最小限の構成に再構築（既存データは破棄）

-- 既存のテーブルを削除
DROP TABLE IF EXISTS firecrawl_articles;

-- 新しいfirecrawl_articlesテーブルを作成
-- urlをプライマリキーとし、rss_articles.linkと等価な状態に
CREATE TABLE IF NOT EXISTS firecrawl_articles (
    url TEXT PRIMARY KEY,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    status_code INTEGER,
    markdown TEXT NOT NULL
);

-- インデックスを作成（created_at用のみ）
CREATE INDEX IF NOT EXISTS idx_firecrawl_articles_created_at ON firecrawl_articles(created_at);