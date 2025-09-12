-- store_article_content競合解決テスト用データ
-- 目的: DISTINCT FROM条件による更新判定とタイムスタンプ更新を検証

-- 既存の記事データ（更新対象）
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://existing.com/article', '2025-01-01 10:00:00+00', 200, 'Original content'),
('https://same.com/article', '2025-01-01 11:00:00+00', 404, 'Error content'),
('https://update.com/article', '2025-01-01 12:00:00+00', 200, 'Will be updated');