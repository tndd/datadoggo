-- ステータスフィルタリングテスト用データ
-- 目的: 成功・エラー・未処理ステータスでの検索動作を検証

-- article_linksテーブル: ステータス別テスト用データ
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://success1.com/article', 'Success Article 1', '2025-01-01 10:00:00+00', 'feed1'),
('https://success2.com/article', 'Success Article 2', '2025-01-02 11:00:00+00', 'feed1'),
('https://error404.com/article', '404 Error Article', '2025-01-03 12:00:00+00', 'feed2'),
('https://error500.com/article', '500 Error Article', '2025-01-04 13:00:00+00', 'feed2'),
('https://unprocessed.com/article', 'Unprocessed Article', '2025-01-05 14:00:00+00', 'feed3');

-- articlesテーブル: 様々なステータスコード
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://success1.com/article', '2025-01-01 10:30:00+00', 200, 'Success content 1'),
('https://success2.com/article', '2025-01-02 11:30:00+00', 200, 'Success content 2'),
('https://error404.com/article', '2025-01-03 12:30:00+00', 404, 'Page not found'),
('https://error500.com/article', '2025-01-04 13:30:00+00', 500, 'Internal server error');
-- unprocessed.com は未処理（articlesテーブルにレコードなし）