-- workflow基本テスト用fixture
-- 8件のarticle_linksのうち、3件は既に処理済み、5件は未処理の状態を作る

-- RSSリンクデータ（8件）
INSERT INTO article_links (url, title, pub_date, source) VALUES 
    -- 未処理になる予定のURL（5件）
    ('https://news.example.com/article1', '基本テスト記事1', '2025-01-01 10:00:00+00:00', 'test'),
    ('https://news.example.com/article2', '基本テスト記事2', '2025-01-01 11:00:00+00:00', 'test'),
    ('https://blog.example.com/post1', 'ブログ記事1', '2025-01-01 12:00:00+00:00', 'test'),
    ('https://tech.example.com/update1', '技術更新1', '2025-01-01 13:00:00+00:00', 'test'),
    ('https://news.example.com/breaking', '速報記事', '2025-01-01 14:00:00+00:00', 'test'),
    
    -- 既に処理済みになる予定のURL（3件）
    ('https://old.example.com/processed1', '処理済み記事1', '2024-12-25 10:00:00+00:00', 'test'),
    ('https://old.example.com/processed2', '処理済み記事2', '2024-12-25 11:00:00+00:00', 'test'),
    ('https://archive.example.com/old', 'アーカイブ記事', '2024-12-20 15:00:00+00:00', 'test');

-- 既に処理済みの記事データ（3件）
INSERT INTO articles (url, timestamp, status_code, content) VALUES 
    ('https://old.example.com/processed1', '2024-12-25 10:30:00+00:00', 200, '処理済み記事1の内容です。'),
    ('https://old.example.com/processed2', '2024-12-25 11:30:00+00:00', 200, '処理済み記事2の内容です。'),
    ('https://archive.example.com/old', '2024-12-20 15:30:00+00:00', 500, '取得エラー: アーカイブ記事はアクセスできませんでした');