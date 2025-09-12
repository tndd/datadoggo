-- 目的別fixture: 空文字content(200)の除外確認

INSERT INTO article_links (url, title, pub_date, source) VALUES
  ('https://empty.test/ok',    'ok',    '2025-01-03 10:00:00+00', 'rss'),
  ('https://empty.test/empty', 'empty', '2025-01-03 11:00:00+00', 'rss');

INSERT INTO articles (url, timestamp, status_code, content) VALUES
  ('https://empty.test/ok',    '2025-01-03 10:05:00+00', 200, 'content exists'),
  ('https://empty.test/empty', '2025-01-03 11:05:00+00', 200, '');

