-- store_article_content関数のSQLロジック検証用テストデータ

-- article_links テーブル: テスト用の基本データ
INSERT INTO article_links (url, title, pub_date, source) VALUES
-- 基本的なテストデータ
('https://logic-test.example.com/basic', 'Basic Logic Test Article', '2025-01-15 12:00:00+00', 'logic'),
('https://logic-test.example.com/conflict', 'Conflict Test Article', '2025-01-15 13:00:00+00', 'logic'),

-- NULL処理テスト用（直接SQLで生成するため、ここでは参照用のみ）
('https://null-test.example.com/reference', 'NULL Test Reference', '2025-01-15 14:00:00+00', 'null_test'),

-- 境界値テスト用  
('https://boundary-test.example.com/status-codes', 'Status Code Boundary Test', '2025-01-15 15:00:00+00', 'boundary'),
('https://boundary-test.example.com/long-urls', 'Long URL Test', '2025-01-15 16:00:00+00', 'boundary'),

-- タイムスタンプ動作テスト用
('https://timestamp-test.example.com/behavior', 'Timestamp Behavior Test', '2025-01-15 17:00:00+00', 'timestamp');

-- articles テーブル: 特定のテストシナリオ用の初期データ
-- （多くのテストケースは実行時に動的に生成されるため、最小限のみ）

-- IS DISTINCT FROM テスト用の既存レコード
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://pre-existing.example.com/distinct-test', '2025-01-15 10:00:00+00', 200, 'Pre-existing content for distinct test');