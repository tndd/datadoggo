-- テスト用のarticle_linksデータ
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://example.com/article1', 'テスト記事1', '2025-01-01 10:00:00+00', 'test-feed'),
('https://example.com/article2', 'テスト記事2', '2025-01-02 11:00:00+00', 'test-feed'),
('https://example.com/article3', 'テスト記事3', '2025-01-03 12:00:00+00', 'other-feed'),
('https://example.com/article4', '古い記事', '2024-12-31 09:00:00+00', 'test-feed'),
('https://different.com/article5', '別ドメインの記事', '2025-01-05 13:00:00+00', 'test-feed');

-- テスト用のarticlesデータ（成功・失敗混在）
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://example.com/article1', '2025-01-01 10:30:00+00', 200, 'テスト記事1の内容です。'),
('https://example.com/article2', '2025-01-02 11:30:00+00', 200, 'テスト記事2の内容です。'),
('https://example.com/article3', '2025-01-03 12:30:00+00', 404, ''),
('https://example.com/article4', '2024-12-31 09:30:00+00', 200, '古い記事の内容です。'),
('https://different.com/article5', '2025-01-05 13:30:00+00', 500, '');