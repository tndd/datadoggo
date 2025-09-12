-- ソースフィルタリングテスト用データ
-- 目的: sourceフィールドでの絞り込み動作を検証

-- article_linksテーブル: 異なるソースでのテスト
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://tech1.com/article', 'Tech Article 1', '2025-01-01 10:00:00+00', 'tech'),
('https://tech2.com/article', 'Tech Article 2', '2025-01-02 11:00:00+00', 'tech'),
('https://news1.com/article', 'News Article 1', '2025-01-03 12:00:00+00', 'news'),
('https://blog1.com/article', 'Blog Article 1', '2025-01-04 13:00:00+00', 'personal-blog');

-- articlesテーブル: 全て成功ステータスで設定
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://tech1.com/article', '2025-01-01 10:30:00+00', 200, 'Tech content 1'),
('https://tech2.com/article', '2025-01-02 11:30:00+00', 200, 'Tech content 2'),
('https://news1.com/article', '2025-01-03 12:30:00+00', 200, 'News content 1'),
('https://blog1.com/article', '2025-01-04 13:30:00+00', 200, 'Blog content 1');