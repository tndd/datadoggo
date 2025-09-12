-- 日付範囲フィルタリングテスト用データ
-- 目的: pub_date_from, pub_date_toでの絞り込み動作を検証

-- article_linksテーブル: 異なる日付での検索テスト用
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://jan01.com/article', 'Jan 01 Article', '2025-01-01 08:00:00+00', 'daily'),
('https://jan15.com/article', 'Jan 15 Article', '2025-01-15 12:00:00+00', 'daily'),
('https://jan31.com/article', 'Jan 31 Article', '2025-01-31 18:00:00+00', 'daily'),
('https://feb01.com/article', 'Feb 01 Article', '2025-02-01 09:00:00+00', 'daily'),
('https://dec31.com/article', 'Dec 31 Article', '2024-12-31 23:59:00+00', 'archive');

-- articlesテーブル: 全て成功ステータスで設定
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://jan01.com/article', '2025-01-01 08:30:00+00', 200, 'Jan 01 content'),
('https://jan15.com/article', '2025-01-15 12:30:00+00', 200, 'Jan 15 content'),
('https://jan31.com/article', '2025-01-31 18:30:00+00', 200, 'Jan 31 content'),
('https://feb01.com/article', '2025-02-01 09:30:00+00', 200, 'Feb 01 content'),
('https://dec31.com/article', '2024-12-31 23:59:30+00', 200, 'Dec 31 content');