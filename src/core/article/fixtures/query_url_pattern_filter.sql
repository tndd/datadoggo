-- URLパターンフィルタリングテスト用データ
-- 目的: ILIKEパターン検索の動作を検証

-- article_linksテーブル: 様々なURLパターンでのテスト
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://tech-blog.com/ai-trends', 'AI Trends 2025', '2025-01-01 10:00:00+00', 'tech'),
('https://news-site.org/politics-update', 'Politics Update', '2025-01-02 11:00:00+00', 'news'),
('https://tech-blog.com/machine-learning', 'ML Guide', '2025-01-03 12:00:00+00', 'tech'),
('https://personal.blog/diary-entry', 'My Diary', '2025-01-04 13:00:00+00', 'personal');

-- articlesテーブル: 全て成功ステータスで設定
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://tech-blog.com/ai-trends', '2025-01-01 10:30:00+00', 200, 'AI content'),
('https://news-site.org/politics-update', '2025-01-02 11:30:00+00', 200, 'Politics content'),
('https://tech-blog.com/machine-learning', '2025-01-03 12:30:00+00', 200, 'ML content'),
('https://personal.blog/diary-entry', '2025-01-04 13:30:00+00', 200, 'Diary content');