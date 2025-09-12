-- 目的別fixture: sourceフィルタの等価比較動作

INSERT INTO article_links (url, title, pub_date, source) VALUES
  ('https://source.test/rss-ok',   'rss ok',   '2025-01-02 10:00:00+00', 'rss'),
  ('https://source.test/manual-ok','man ok',   '2025-01-02 11:00:00+00', 'manual');

INSERT INTO articles (url, timestamp, status_code, content) VALUES
  ('https://source.test/rss-ok',    '2025-01-02 10:05:00+00', 200, 'rss ok'),
  ('https://source.test/manual-ok', '2025-01-02 11:05:00+00', 200, 'man ok');

