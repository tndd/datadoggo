-- 目的別fixture: ステータスフィルタ検証（未処理/成功/404）

INSERT INTO article_links (url, title, pub_date, source) VALUES
  ('https://stat.test/success',    'ok',     '2025-01-01 10:00:00+00', 'rss'),
  ('https://stat.test/error-404',  'nf',     '2025-01-01 11:00:00+00', 'rss'),
  ('https://stat.test/unprocessed','wait',   '2025-01-01 12:00:00+00', 'rss');

INSERT INTO articles (url, timestamp, status_code, content) VALUES
  ('https://stat.test/success',    '2025-01-01 10:05:00+00', 200, 'ok'),
  ('https://stat.test/error-404',  '2025-01-01 11:05:00+00', 404, 'not found');

