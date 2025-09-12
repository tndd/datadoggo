-- 目的別fixture: 期間境界(>=, <=)の包含確認

INSERT INTO article_links (url, title, pub_date, source) VALUES
  ('https://date.test/before',       'before',       '2024-12-31 23:59:59+00', 'rss'),
  ('https://date.test/exact-start',  'exact-start',  '2025-01-01 00:00:00+00', 'rss'),
  ('https://date.test/exact-end',    'exact-end',    '2025-01-01 23:59:59+00', 'rss');

INSERT INTO articles (url, timestamp, status_code, content) VALUES
  ('https://date.test/before',       '2025-01-01 00:00:10+00', 200, 'x'),
  ('https://date.test/exact-start',  '2025-01-01 00:01:00+00', 200, 'y'),
  ('https://date.test/exact-end',    '2025-01-01 23:59:59+00', 200, 'z');

