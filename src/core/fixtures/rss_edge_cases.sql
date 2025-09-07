-- エッジケース機能テスト用fixture
-- 境界値、特殊なデータ構造、異常な入力値での動作確認

-- 空文字・NULL類似のテスト
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://empty-title.example.com/article', '', '2025-01-20T10:00:00Z', 'test'),
('https://minimal-title.example.com/article', 'a', '2025-01-20T11:00:00Z', 'test');

-- 境界値：非常に短いURL vs 長いURL
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://x.co/1', '最短URL記事', '2025-01-20T12:00:00Z', 'test'),
('https://very-long-subdomain-for-testing-boundary-conditions.example-domain-name.com/path/to/article/with/very/long/path/segments/article.html?param1=value1&param2=value2', 
'長いURL記事', '2025-01-20T13:00:00Z', 'test');

-- 日付境界値：UNIXエポック近辺、Y2K問題、うるう年
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://epoch.example.com/unix-start', 'UNIX開始日記事', '1970-01-01T00:00:01Z', 'test'),
('https://y2k.example.com/problem', 'Y2K問題記事', '2000-01-01T00:00:00Z', 'test'),
('https://leap.example.com/year', 'うるう年記事', '2024-02-29T12:00:00Z', 'test'),
('https://precision.example.com/microsecond', 'マイクロ秒精度記事', '2025-01-20T14:30:45.123456Z', 'test');

-- 重複処理テスト用の既存記事
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://duplicate-test-edge-case.example.com/article', '重複テスト記事', '2025-01-20T15:00:00Z', 'source-1');

-- 対応する既存記事（UPSERT動作確認用）
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://duplicate-test-edge-case.example.com/article', '2025-01-20T15:30:00Z', 200, '最初の記事内容');

-- ソート順の境界条件：同一日時の記事複数
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://same-time-1.example.com/article', '同時刻記事1', '2025-01-20T16:00:00Z', 'test'),
('https://same-time-2.example.com/article', '同時刻記事2', '2025-01-20T16:00:00Z', 'test'),
('https://same-time-3.example.com/article', '同時刻記事3', '2025-01-20T16:00:00Z', 'test');

-- URL、タイトルでの検索境界条件
INSERT INTO article_links (url, title, pub_date, source) VALUES
-- 部分一致の境界
('https://search-boundary.example.com/exact', '完全一致テスト', '2025-01-20T17:00:00Z', 'test'),
('https://search-boundary.example.com/exact-longer', '完全一致テスト延長', '2025-01-20T17:01:00Z', 'test'),
('https://different-search-boundary.example.com/exact', '異なるドメイン完全一致', '2025-01-20T17:02:00Z', 'test'),

-- 大文字小文字混在（ILIKE動作確認）
('https://CaseSensitive.Example.COM/Article', 'Mixed Case Test', '2025-01-20T18:00:00Z', 'test'),
('https://casesensitive.example.com/article', 'Lower Case Version', '2025-01-20T18:01:00Z', 'test');

-- source フィールドの境界値
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://source-test.example.com/empty', 'Empty Source Test', '2025-01-20T19:00:00Z', ''),
('https://source-test.example.com/single', 'Single Char Source', '2025-01-20T19:01:00Z', 'x'),
('https://source-test.example.com/long', 'Long Source Name Test', '2025-01-20T19:02:00Z', 'very-long-source-name-for-boundary-testing');

-- 日時フィルタの境界条件確認用記事
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://filter-test.example.com/start-exact', '開始時刻ピッタリ', '2025-01-15T00:00:00Z', 'filter-test'),
('https://filter-test.example.com/end-exact', '終了時刻ピッタリ', '2025-01-15T23:59:59Z', 'filter-test'),
('https://filter-test.example.com/before-range', '範囲前', '2025-01-14T23:59:59Z', 'filter-test'),
('https://filter-test.example.com/after-range', '範囲後', '2025-01-16T00:00:01Z', 'filter-test'),
('https://filter-test.example.com/middle', '範囲内', '2025-01-15T12:00:00Z', 'filter-test');

-- バックログ検索の境界条件：articles テーブルとの LEFT JOIN
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://backlog-test.example.com/unprocessed-1', '未処理記事1', '2025-01-20T20:00:00Z', 'backlog'),
('https://backlog-test.example.com/unprocessed-2', '未処理記事2', '2025-01-20T20:01:00Z', 'backlog'),
('https://backlog-test.example.com/processed-success', '処理済み成功記事', '2025-01-20T20:02:00Z', 'backlog'),
('https://backlog-test.example.com/processed-error', '処理済みエラー記事', '2025-01-20T20:03:00Z', 'backlog');

-- 対応する articles データ（バックログテスト用）
INSERT INTO articles (url, timestamp, status_code, content) VALUES
-- 成功記事（バックログ対象外）
('https://backlog-test.example.com/processed-success', '2025-01-20T20:32:00Z', 200, '成功記事内容'),
-- エラー記事（バックログ対象に含まれる）
('https://backlog-test.example.com/processed-error', '2025-01-20T20:33:00Z', 500, 'エラー記事内容');

-- store_article_links のバルク処理境界条件
-- 空配列、単一要素、重複混在をテスト
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://bulk-test.example.com/single', '単一バルク記事', '2025-01-20T21:00:00Z', 'bulk-test');

-- 既存記事との更新条件確認
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://bulk-test.example.com/single', '2025-01-20T21:30:00Z', 200, '既存のバルク記事内容');