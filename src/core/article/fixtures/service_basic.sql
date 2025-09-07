-- 基本的なテストデータ（基本的な動作確認用）

-- article_links テーブルの基本データ
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://example.com/article1', 'テスト記事1', '2025-01-01 10:00:00+00', 'test-feed'),
('https://example.com/article2', 'テスト記事2', '2025-01-02 11:00:00+00', 'test-feed'),
('https://example.com/article3', 'テスト記事3', '2025-01-03 12:00:00+00', 'other-feed');

-- articles テーブルの基本データ
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://example.com/article1', '2025-01-01 10:30:00+00', 200, 'テスト記事1の内容です。'),
('https://example.com/article2', '2025-01-02 11:30:00+00', 200, 'テスト記事2の内容です。'),
('https://example.com/article3', '2025-01-03 12:30:00+00', 404, '');