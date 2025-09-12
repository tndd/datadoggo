-- 検索・パターンマッチング用テストデータ

-- article_links テーブルのパターン検索用データ
INSERT INTO article_links (url, title, pub_date, source) VALUES
-- 様々なドメインパターン
('https://tech.example.com/rust-tutorial', 'Rust チュートリアル', '2025-01-10 14:00:00+00', 'tech-feed'),
('https://tech.example.com/python-guide', 'Python ガイド', '2025-01-11 15:00:00+00', 'tech-feed'),
('https://news.example.com/breaking-news', '速報ニュース', '2025-01-12 16:00:00+00', 'news-feed'),
('https://blog.different.org/post1', 'ブログ記事1', '2025-01-13 17:00:00+00', 'blog-feed'),

-- パス部分のパターン
('https://example.com/tutorial/advanced', 'Advanced Tutorial', '2025-01-14 18:00:00+00', 'tutorial-feed'),
('https://example.com/guide/beginner', 'Beginner Guide', '2025-01-15 19:00:00+00', 'guide-feed');

-- articles テーブルのパターン検索用データ
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://tech.example.com/rust-tutorial', '2025-01-10 14:30:00+00', 200, 'Rustの詳細なチュートリアル内容です。'),
('https://tech.example.com/python-guide', '2025-01-11 15:30:00+00', 200, 'Pythonガイドの詳細な内容です。'),
('https://news.example.com/breaking-news', '2025-01-12 16:30:00+00', 200, '重要なニュースの内容です。'),
('https://blog.different.org/post1', '2025-01-13 17:30:00+00', 403, 'アクセス拒否'),
('https://example.com/tutorial/advanced', '2025-01-14 18:30:00+00', 200, 'Advanced tutorial content'),
('https://example.com/guide/beginner', '2025-01-15 19:30:00+00', 200, 'Beginner guide content');