-- 目的別fixture: URLパターン + LIMIT + ORDER ASC(by url)

INSERT INTO article_links (url, title, pub_date, source) VALUES
  ('https://limit.test/a', 'a', '2025-01-01 00:00:00+00', 'rss'),
  ('https://limit.test/b', 'b', '2025-01-01 00:00:01+00', 'rss'),
  ('https://limit.test/c', 'c', '2025-01-01 00:00:02+00', 'rss'),
  ('https://limit.test/d', 'd', '2025-01-01 00:00:03+00', 'rss');

-- c,d は記事有無が混在しても良いが、URL昇順とLIMITの確認が主目的
INSERT INTO articles (url, timestamp, status_code, content) VALUES
  ('https://limit.test/a', '2025-01-01 00:10:00+00', 200, 'A'),
  ('https://limit.test/b', '2025-01-01 00:10:00+00', 200, 'B');

