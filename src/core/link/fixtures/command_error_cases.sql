-- エラーシナリオテスト用のデータ
-- 記事取得時に失敗ケースや異常なデータを含むテスト用fixture

-- 基本的な記事リンク（正常ケース）
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://success.example.com/normal-article', '正常な記事', '2025-01-20T10:00:00Z', 'test'),
('https://success.example.com/another-normal', 'もう一つの正常記事', '2025-01-20T11:00:00Z', 'test');

-- 既存の成功記事（更新テスト用）
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://success.example.com/normal-article', '2025-01-20T10:30:00Z', 200, '既存の成功記事内容');

-- 既存のエラー記事（再処理テスト用）
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://error.example.com/timeout-article', 'タイムアウト記事', '2025-01-20T12:00:00Z', 'test'),
('https://error.example.com/forbidden-article', '403エラー記事', '2025-01-20T13:00:00Z', 'test'),
('https://error.example.com/notfound-article', '404エラー記事', '2025-01-20T14:00:00Z', 'test');

-- 既存のエラー記事データ（再処理対象）
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://error.example.com/timeout-article', '2025-01-20T12:30:00Z', 500, 'タイムアウトエラー'),
('https://error.example.com/forbidden-article', '2025-01-20T13:30:00Z', 403, 'アクセス禁止'),
('https://error.example.com/notfound-article', '2025-01-20T14:30:00Z', 404, 'ページが見つかりません');

-- 長いURL・タイトルのエッジケース
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://very-long-domain-name-for-testing-purposes.example.com/extremely/long/path/to/article/with/many/segments/and/parameters?param1=value1&param2=value2&param3=verylongvalue', 
'非常に長いタイトルのテスト記事です。このタイトルは通常より長く、システムが適切に処理できるかを確認するためのものです。', 
'2025-01-20T15:00:00Z', 'test');

-- 特殊文字を含むURL・タイトル
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://special.example.com/article%20with%20encoded%20spaces?query=%E3%83%86%E3%82%B9%E3%83%88', 
'特殊文字テスト: "引用符" & アンパサンド < > 不等号 [brackets] {braces}', 
'2025-01-20T16:00:00Z', 'test'),
('https://unicode.example.com/記事/テスト?クエリ=値', 
'Unicodeテスト: 日本語🗾 English 한국어 العربية', 
'2025-01-20T17:00:00Z', 'test');

-- 日付境界値テスト
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://boundary.example.com/very-old-article', '非常に古い記事', '2000-01-01T00:00:00Z', 'test'),
('https://boundary.example.com/future-article', '未来の記事', '2030-12-31T23:59:59Z', 'test');

-- 異なるソース値のテスト
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://source1.example.com/article', 'ソース1の記事', '2025-01-20T18:00:00Z', 'rss-feed-1'),
('https://source2.example.com/article', 'ソース2の記事', '2025-01-20T18:30:00Z', 'rss-feed-2'),
('https://source3.example.com/article', 'ソース3の記事', '2025-01-20T19:00:00Z', 'manual');

-- UPSERT確認用（重複は後でテスト内で作成）
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://duplicate-test-error-case.example.com/first-insert', '最初のタイトル', '2025-01-20T20:00:00Z', 'test');