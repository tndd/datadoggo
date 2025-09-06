-- フィルタリングテスト用のデータ
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://tech.example.com/rust-tutorial', 'Rust チュートリアル', '2025-01-10 14:00:00+00', 'tech-feed'),
('https://tech.example.com/python-guide', 'Python ガイド', '2025-01-11 15:00:00+00', 'tech-feed'),
('https://news.example.com/breaking-news', '速報ニュース', '2025-01-12 16:00:00+00', 'news-feed'),
('https://tech.example.com/old-article', '古い技術記事', '2024-12-01 08:00:00+00', 'tech-feed');

INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://tech.example.com/rust-tutorial', '2025-01-10 14:30:00+00', 200, 'Rustの詳細なチュートリアル内容です。'),
('https://tech.example.com/python-guide', '2025-01-11 15:30:00+00', 200, 'Pythonガイドの詳細な内容です。'),
('https://news.example.com/breaking-news', '2025-01-12 16:30:00+00', 200, '重要なニュースの内容です。'),
('https://tech.example.com/old-article', '2024-12-01 08:30:00+00', 200, '古い技術記事の内容です。');