-- RSS記事の基本テストデータ（シンプルな保存・取得機能のテスト用）
INSERT INTO rss_articles (link, title, description, pub_date) VALUES
-- 基本的なテストデータ（異なる時刻）
('https://test.example.com/article1', 'テスト記事1', 'これはテスト記事の説明1です', '2025-08-26T10:00:00Z'),
('https://test.example.com/article2', 'テスト記事2', 'これはテスト記事の説明2です', '2025-08-26T11:00:00Z'),

-- 異なるドメインの記事
('https://news.sample.org/breaking', 'ニュース記事', '重要なニュースの内容', '2025-08-26T09:30:00Z'),
('https://blog.tech.net/update', '技術ブログ', 'プログラミングに関する記事', '2025-08-26T12:15:00Z'),

-- descriptionがNULLの記事
('https://minimal.site.com/simple', 'シンプル記事', NULL, '2025-08-26T13:45:00Z'),

-- 古い日付の記事
('https://archive.old.com/legacy', '過去の記事', '古いアーカイブ記事', '2025-08-25T08:00:00Z');