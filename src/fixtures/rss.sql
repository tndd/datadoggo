-- RSS記事の統合テストデータ（全テストシナリオ対応）
INSERT INTO rss_articles (link, title, description, pub_date) VALUES
-- 基本テストデータ（save_testsで重複テスト用として使用）
('https://test.example.com/article1', 'テスト記事1', 'これはテスト記事の説明1です', '2025-08-26T10:00:00Z'),
('https://test.example.com/article2', 'テスト記事2', 'これはテスト記事の説明2です', '2025-08-26T11:00:00Z'),
('https://news.sample.org/breaking', 'ニュース記事', '重要なニュースの内容', '2025-08-26T09:30:00Z'),
('https://blog.tech.net/update', '技術ブログ', 'プログラミングに関する記事', '2025-08-26T12:15:00Z'),

-- 日付境界テスト用（厳密な境界値）
('https://test.com/boundary/exactly-start', 'Boundary Start Article', 'Exactly at start time', '2025-01-15T00:00:00Z'),
('https://test.com/boundary/exactly-end', 'Boundary End Article', 'Exactly at end time', '2025-01-15T23:59:59Z'),
('https://test.com/boundary/one-second-before', 'Just Before Range', 'One second before range', '2025-01-14T23:59:59Z'),
('https://test.com/boundary/one-second-after', 'Just After Range', 'One second after range', '2025-01-16T00:00:01Z'),

-- 取得機能テスト用
('https://example.com/tech/article-2025-01-15', 'Tech News 2025', 'Latest technology updates', '2025-01-15T10:00:00Z'),
('https://news.example.com/world/breaking-news', 'Breaking World News', 'Important global events', '2025-01-14T15:30:00Z'),
('https://blog.example.com/lifestyle/health-tips', 'Health Tips for 2025', 'Wellness and fitness advice', '2025-01-16T08:45:00Z'),

-- URL部分一致テスト用
('https://example.com/test', 'Simple Test', 'Basic test article', '2025-01-10T12:00:00Z'),
('https://not-example.com/test', 'Not Example Test', 'Different domain test', '2025-01-11T12:00:00Z'),

-- 特殊文字・エスケープテスト用
('https://special.com/article%20with%20spaces', 'Article With Spaces', 'URL encoding test', '2025-01-13T12:00:00Z'),
('https://special.com/article_with_underscore', 'Article With Underscore', 'Underscore in URL', '2025-01-13T13:00:00Z'),

-- 大小文字混合テスト用（ILIKE確認）
('https://CaseSensitive.com/MixedCase', 'Mixed Case Article', 'Case sensitivity test', '2025-01-14T12:00:00Z'),

-- NULL値処理テスト用
('https://minimal.site.com/simple', 'シンプル記事', NULL, '2025-08-26T13:45:00Z'),
('https://null-test.com/no-description', 'No Description Article', NULL, '2025-01-17T12:00:00Z'),
('https://null-test.com/no-date', 'No Date Article', 'Article without date', NULL);