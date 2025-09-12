-- フィルタリングテスト用のデータ
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://tech.example.com/rust-tutorial', 'Rust チュートリアル', '2025-01-10 14:00:00+00', 'tech-feed'),
('https://tech.example.com/python-guide', 'Python ガイド', '2025-01-11 15:00:00+00', 'tech-feed'),
('https://news.example.com/breaking-news', '速報ニュース', '2025-01-12 16:00:00+00', 'news-feed'),
('https://tech.example.com/old-article', '古い技術記事', '2024-12-01 08:00:00+00', 'tech-feed'),
-- 境界値・limit テスト用データ
('https://tech.example.com/article1', '技術記事1', '2025-01-01 00:00:00+00', 'tech-feed'),
('https://tech.example.com/article2', '技術記事2', '2025-12-31 23:59:59+00', 'tech-feed'),
('https://other.example.com/mixed1', '混合記事1', '2025-06-15 12:00:00+00', 'mixed-feed'),
('https://tech.example.com/no-content', 'コンテンツなし記事', '2025-01-15 10:00:00+00', 'tech-feed'),
('https://tech.example.com/limit-test1', 'Limit テスト 1', '2025-02-01 09:00:00+00', 'tech-feed'),
('https://tech.example.com/limit-test2', 'Limit テスト 2', '2025-02-02 09:00:00+00', 'tech-feed'),
('https://tech.example.com/limit-test3', 'Limit テスト 3', '2025-02-03 09:00:00+00', 'tech-feed'),
('https://tech.example.com/limit-test4', 'Limit テスト 4', '2025-02-04 09:00:00+00', 'tech-feed'),
('https://tech.example.com/limit-test5', 'Limit テスト 5', '2025-02-05 09:00:00+00', 'tech-feed'),
('https://tech.example.com/limit-test6', 'Limit テスト 6', '2025-02-06 09:00:00+00', 'tech-feed');

INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://tech.example.com/rust-tutorial', '2025-01-10 14:30:00+00', 200, 'Rustの詳細なチュートリアル内容です。'),
('https://tech.example.com/python-guide', '2025-01-11 15:30:00+00', 200, 'Pythonガイドの詳細な内容です。'),
('https://news.example.com/breaking-news', '2025-01-12 16:30:00+00', 200, '重要なニュースの内容です。'),
('https://tech.example.com/old-article', '2024-12-01 08:30:00+00', 200, '古い技術記事の内容です。'),
-- 境界値・limit テスト用データ
('https://tech.example.com/article1', '2025-01-01 00:30:00+00', 200, '境界値テスト: 年始の記事'),
('https://tech.example.com/article2', '2025-12-31 23:59:59+00', 200, '境界値テスト: 年末の記事'),
('https://other.example.com/mixed1', '2025-06-15 12:30:00+00', 200, '異なるドメインの記事内容'),
('https://tech.example.com/no-content', '2025-01-15 10:30:00+00', 404, ''),
('https://tech.example.com/limit-test1', '2025-02-01 09:30:00+00', 200, 'Limit テスト用記事1の内容'),
('https://tech.example.com/limit-test2', '2025-02-02 09:30:00+00', 200, 'Limit テスト用記事2の内容'),
('https://tech.example.com/limit-test3', '2025-02-03 09:30:00+00', 200, 'Limit テスト用記事3の内容'),
('https://tech.example.com/limit-test4', '2025-02-04 09:30:00+00', 200, 'Limit テスト用記事4の内容'),
('https://tech.example.com/limit-test5', '2025-02-05 09:30:00+00', 200, 'Limit テスト用記事5の内容'),
('https://tech.example.com/limit-test6', '2025-02-06 09:30:00+00', 200, 'Limit テスト用記事6の内容');
