CREATE TABLE IF NOT EXISTS articles (
    title       TEXT NOT NULL,
    link        TEXT PRIMARY KEY,
    description TEXT,
    pub_date    TEXT
);