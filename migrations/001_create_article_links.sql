-- 記事リンク用テーブル（ArticleLink構造に対応）
CREATE TABLE article_links (
    url TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    pub_date TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL
);