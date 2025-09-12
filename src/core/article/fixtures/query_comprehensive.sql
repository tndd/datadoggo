-- repository.rsの詳細なテスト用データ

-- article_links テーブル: 様々なソースとパターン
INSERT INTO article_links (url, title, pub_date, source) VALUES
-- tech ソース
('https://tech-news.com/article1', 'Tech Article 1', '2025-03-01 10:00:00+00', 'tech'),
('https://tech-news.com/article2', 'Tech Article 2', '2025-03-02 11:00:00+00', 'tech'),
('https://techblog.example.com/post1', 'Tech Blog Post', '2025-03-03 12:00:00+00', 'tech'),

-- news ソース  
('https://news-daily.com/story1', 'Daily News Story 1', '2025-03-04 13:00:00+00', 'news'),
('https://news-daily.com/story2', 'Daily News Story 2', '2025-03-05 14:00:00+00', 'news'),

-- blog ソース
('https://personal-blog.net/entry1', 'Personal Blog Entry 1', '2025-03-06 15:00:00+00', 'blog'),
('https://personal-blog.net/entry2', 'Personal Blog Entry 2', '2025-03-07 16:00:00+00', 'blog'),

-- 異なる日付範囲でのテスト用
('https://old.example.com/ancient', 'Ancient Article', '2024-12-01 08:00:00+00', 'archive'),
('https://future.example.com/upcoming', 'Future Article', '2025-12-31 23:59:00+00', 'future'),

-- 特殊なURLパターン
('https://sub.domain.example.com/deep/path/article', 'Deep Path Article', '2025-03-10 10:00:00+00', 'special'),
('https://example-site.org/simple', 'Simple Article', '2025-03-11 11:00:00+00', 'simple');

-- articles テーブル: 様々なステータスコードとコンテンツパターン
INSERT INTO articles (url, timestamp, status_code, content) VALUES
-- 成功記事 (200)
('https://tech-news.com/article1', '2025-03-01 10:30:00+00', 200, 'Detailed technical content about latest technologies and innovations.'),
('https://tech-news.com/article2', '2025-03-02 11:30:00+00', 200, 'Advanced programming techniques and best practices.'),
('https://news-daily.com/story1', '2025-03-04 13:30:00+00', 200, 'Breaking news coverage with comprehensive analysis.'),

-- クライアントエラー (4xx)
('https://techblog.example.com/post1', '2025-03-03 12:30:00+00', 404, 'Page not found'),
('https://news-daily.com/story2', '2025-03-05 14:30:00+00', 403, 'Access forbidden'),

-- サーバーエラー (5xx)  
('https://personal-blog.net/entry1', '2025-03-06 15:30:00+00', 500, 'Internal server error occurred'),
('https://personal-blog.net/entry2', '2025-03-07 16:30:00+00', 502, 'Bad gateway error'),

-- その他のステータス
('https://example-site.org/simple', '2025-03-11 11:30:00+00', 301, 'Moved permanently'),

-- 特殊コンテンツパターン
('https://old.example.com/ancient', '2024-12-01 08:30:00+00', 200, ''),  -- 空のコンテンツ
('https://sub.domain.example.com/deep/path/article', '2025-03-10 10:30:00+00', 200, 'Special characters: 日本語 中文 한글 العربية 🚀 🌟 💻 <script>alert("test")</script> SELECT * FROM users; DROP TABLE--');