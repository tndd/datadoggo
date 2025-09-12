-- 汎用fixture: 成功/エラー/未処理、URLパターン、期間/ソース/空contentを網羅

-- article_links（6件）
INSERT INTO article_links (url, title, pub_date, source) VALUES
  ('https://example.com/article-1', 'OK Article',        '2025-01-10 12:00:00+00', 'rss'),
  ('https://example.com/article-2', 'Err Article',       '2025-01-11 13:00:00+00', 'rss'),
  ('https://example.com/article-3', 'Unprocessed',       '2025-01-12 10:00:00+00', 'rss'),
  ('https://another.com/path/ok',   'Another OK',        '2025-02-01 10:30:00+00', 'manual'),
  ('https://another.com/path/empty','Empty Content',     '2025-02-01 11:00:00+00', 'manual'),
  ('https://source-only.com/rss/ok','RSS Source Only',   '2025-01-15 09:00:00+00', 'rss');

-- articles（成功/エラー/空content）
INSERT INTO articles (url, timestamp, status_code, content) VALUES
  ('https://example.com/article-1', '2025-01-10 12:30:00+00', 200, 'Content 1'),
  ('https://example.com/article-2', '2025-01-11 13:30:00+00', 500, 'Server error content'),
  -- article-3 は未処理のまま
  ('https://another.com/path/ok',    '2025-02-01 10:45:00+00', 200, 'Another content'),
  ('https://another.com/path/empty', '2025-02-01 11:10:00+00', 200, ''),
  ('https://source-only.com/rss/ok', '2025-01-15 09:05:00+00', 200, 'RSS only content');

