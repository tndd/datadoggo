-- limit機能テスト用データ

-- article_links テーブルのlimitテスト用データ
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://limit.test.com/item01', 'Limit テスト 01', '2025-02-01 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item02', 'Limit テスト 02', '2025-02-02 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item03', 'Limit テスト 03', '2025-02-03 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item04', 'Limit テスト 04', '2025-02-04 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item05', 'Limit テスト 05', '2025-02-05 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item06', 'Limit テスト 06', '2025-02-06 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item07', 'Limit テスト 07', '2025-02-07 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item08', 'Limit テスト 08', '2025-02-08 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item09', 'Limit テスト 09', '2025-02-09 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item10', 'Limit テスト 10', '2025-02-10 09:00:00+00', 'limit-feed');

-- articles テーブルのlimitテスト用データ（一部のみ成功、順序確認用）
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://limit.test.com/item01', '2025-02-01 09:30:00+00', 200, 'Limit テスト用記事1の内容'),
('https://limit.test.com/item02', '2025-02-02 09:30:00+00', 200, 'Limit テスト用記事2の内容'),
('https://limit.test.com/item03', '2025-02-03 09:30:00+00', 404, ''),
('https://limit.test.com/item04', '2025-02-04 09:30:00+00', 200, 'Limit テスト用記事4の内容'),
('https://limit.test.com/item05', '2025-02-05 09:30:00+00', 500, 'サーバーエラー'),
('https://limit.test.com/item06', '2025-02-06 09:30:00+00', 200, 'Limit テスト用記事6の内容'),
('https://limit.test.com/item07', '2025-02-07 09:30:00+00', 200, 'Limit テスト用記事7の内容');