-- repository.rsのエッジケース・境界値テスト用データ

-- article_links テーブル: エッジケース用データ
INSERT INTO article_links (url, title, pub_date, source) VALUES
-- 境界日時 (年境界、月境界)
('https://boundary.test.com/year-end', 'Year End Article', '2024-12-31 23:59:59+00', 'boundary'),
('https://boundary.test.com/year-start', 'Year Start Article', '2025-01-01 00:00:00+00', 'boundary'),
('https://boundary.test.com/month-end', 'Month End Article', '2025-01-31 23:59:59+00', 'boundary'),
('https://boundary.test.com/month-start', 'Month Start Article', '2025-02-01 00:00:00+00', 'boundary'),

-- 極端に長いURL/タイトル
('https://extremely-long-domain-name-for-testing-purposes.example.com/very/long/path/with/many/segments/and/parameters?param1=value1&param2=value2&param3=value3', 'This is an extremely long title that tests the limits of our system and ensures that we can handle edge cases properly', '2025-03-15 12:00:00+00', 'edge'),

-- 特殊文字を含むURL/タイトル
('https://special.test.com/カタカナ/ひらがな/漢字', '日本語タイトル: "引用符"とアンド&記号', '2025-03-16 13:00:00+00', 'special'),
('https://unicode.test.com/emoji/🚀🌟💻', 'Unicode Title with Emoji 🎉 and Symbols ™®©', '2025-03-17 14:00:00+00', 'unicode'),

-- 空に近いデータ
('https://minimal.test.com/empty-title', '', '2025-03-18 15:00:00+00', ''),
('https://single-char.test.com/a', 'A', '2025-03-19 16:00:00+00', 'x'),

-- 重複テスト用
('https://duplicate-test.example.com/original', 'Original Entry', '2025-03-20 17:00:00+00', 'dup'),
('https://duplicate-test.example.com/updated', 'Updated Entry', '2025-03-21 18:00:00+00', 'dup');

-- articles テーブル: エッジケース用データ  
INSERT INTO articles (url, timestamp, status_code, content) VALUES
-- 境界日時での更新
('https://boundary.test.com/year-end', '2024-12-31 23:59:59+00', 200, 'Content at year boundary'),
('https://boundary.test.com/year-start', '2025-01-01 00:00:01+00', 200, 'Content at year start'),

-- 極端に大きな/小さなステータスコード
('https://boundary.test.com/month-end', '2025-01-31 23:59:59+00', 100, 'Continue response'),
('https://boundary.test.com/month-start', '2025-02-01 00:00:01+00', 599, 'Custom server error'),

-- 極端に長いコンテンツ
('https://extremely-long-domain-name-for-testing-purposes.example.com/very/long/path/with/many/segments/and/parameters?param1=value1&param2=value2&param3=value3', '2025-03-15 12:30:00+00', 200, REPEAT('This is a very long content string that will be repeated many times to test the system limits and memory usage patterns. ', 1000)),

-- 特殊文字コンテンツ  
('https://special.test.com/カタカナ/ひらがな/漢字', '2025-03-16 13:30:00+00', 200, 'マルチバイト文字のテスト: 🚀🌟💻 
改行文字テスト
タブ文字テスト	ここ
NULL文字テスト'),

-- SQLインジェクション対策テスト
('https://unicode.test.com/emoji/🚀🌟💻', '2025-03-17 14:30:00+00', 200, 'SQL injection attempt: ''; DROP TABLE articles; --
XSS attempt: <script>alert("hack")</script>
Path traversal: ../../../etc/passwd
Command injection: $(rm -rf /)'),

-- 空/NULL風のコンテンツ
('https://minimal.test.com/empty-title', '2025-03-18 15:30:00+00', 200, ''),
('https://single-char.test.com/a', '2025-03-19 16:30:00+00', 404, 'null'),

-- 重複更新テスト用
('https://duplicate-test.example.com/original', '2025-03-20 17:30:00+00', 200, 'Original content that will be updated'),
('https://duplicate-test.example.com/updated', '2025-03-21 18:30:00+00', 200, 'Updated content after modification');