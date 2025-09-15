-- search_backlog_article_links専用テストfixture
-- バックログ記事（未処理 + エラー記事）の取得機能を包括的にテストするためのデータ

-- article_linksデータ（12件）
INSERT INTO article_links (url, title, pub_date, source) VALUES
    -- 未処理記事（5件）: articlesテーブルに存在しない = バックログ対象
    ('https://unprocessed.example.com/article1', '未処理記事1', '2025-01-01 10:00:00+00:00', 'rss'),
    ('https://unprocessed.example.com/article2', '未処理記事2', '2025-01-01 11:00:00+00:00', 'rss'),
    ('https://unprocessed.example.com/article3', '未処理記事3', '2025-01-01 12:00:00+00:00', 'manual'),
    ('https://unprocessed.example.com/article4', '未処理記事4', '2025-01-01 13:00:00+00:00', 'rss'),
    ('https://unprocessed.example.com/article5', '未処理記事5', '2025-01-01 14:00:00+00:00', 'rss'),

    -- エラー記事（4件）: status_code != 200 = バックログ対象
    ('https://error.example.com/not-found', '404エラー記事', '2025-01-02 10:00:00+00:00', 'rss'),
    ('https://error.example.com/server-error', '500エラー記事', '2025-01-02 11:00:00+00:00', 'rss'),
    ('https://error.example.com/bad-gateway', '502エラー記事', '2025-01-02 12:00:00+00:00', 'manual'),
    ('https://error.example.com/forbidden', '403エラー記事', '2025-01-02 13:00:00+00:00', 'rss'),

    -- 成功記事（3件）: status_code = 200 = バックログ対象外
    ('https://success.example.com/article1', '成功記事1', '2025-01-03 10:00:00+00:00', 'rss'),
    ('https://success.example.com/article2', '成功記事2', '2025-01-03 11:00:00+00:00', 'manual'),
    ('https://success.example.com/article3', '成功記事3', '2025-01-03 12:00:00+00:00', 'rss');

-- articlesデータ（7件）
-- 注意: 未処理記事5件はこのテーブルに存在しない
INSERT INTO articles (url, timestamp, status_code, content) VALUES
    -- エラー記事（4件）
    ('https://error.example.com/not-found', '2025-01-02 10:30:00+00:00', 404, '取得エラー: Page not found'),
    ('https://error.example.com/server-error', '2025-01-02 11:30:00+00:00', 500, '取得エラー: Internal server error'),
    ('https://error.example.com/bad-gateway', '2025-01-02 12:30:00+00:00', 502, '取得エラー: Bad gateway'),
    ('https://error.example.com/forbidden', '2025-01-02 13:30:00+00:00', 403, '取得エラー: Forbidden access'),

    -- 成功記事（3件）
    ('https://success.example.com/article1', '2025-01-03 10:30:00+00:00', 200, '成功記事1の内容です'),
    ('https://success.example.com/article2', '2025-01-03 11:30:00+00:00', 200, '成功記事2の内容です'),
    ('https://success.example.com/article3', '2025-01-03 12:30:00+00:00', 200, '成功記事3の内容です');

-- 期待されるバックログ記事数: 9件（未処理5件 + エラー4件）
-- 除外される記事数: 3件（成功3件）