-- service.rs専用の包括的テストデータ
-- 検索、フィルタリング、境界値、エッジケースを含む

-- article_links テーブルのテストデータ
INSERT INTO article_links (url, title, pub_date, source) VALUES
-- 基本的なテストデータ
('https://example.com/article1', 'テスト記事1', '2025-01-01 10:00:00+00', 'test-feed'),
('https://example.com/article2', 'テスト記事2', '2025-01-02 11:00:00+00', 'test-feed'),
('https://example.com/article3', 'テスト記事3', '2025-01-03 12:00:00+00', 'other-feed'),

-- 検索テスト用データ（様々なドメイン）
('https://tech.example.com/rust-tutorial', 'Rust チュートリアル', '2025-01-10 14:00:00+00', 'tech-feed'),
('https://tech.example.com/python-guide', 'Python ガイド', '2025-01-11 15:00:00+00', 'tech-feed'),
('https://news.example.com/breaking-news', '速報ニュース', '2025-01-12 16:00:00+00', 'news-feed'),
('https://blog.different.org/post1', 'ブログ記事1', '2025-01-13 17:00:00+00', 'blog-feed'),

-- 境界値テスト用データ
('https://boundary.test.com/start', '境界値: 年始', '2025-01-01 00:00:00+00', 'boundary-feed'),
('https://boundary.test.com/end', '境界値: 年末', '2025-12-31 23:59:59+00', 'boundary-feed'),
('https://boundary.test.com/middle', '境界値: 中間', '2025-06-15 12:00:00+00', 'boundary-feed'),

-- 特殊文字テスト用データ
('https://special.example.com/chars', 'Title with "quotes" & <tags>', '2025-01-06 14:00:00+00', 'special-feed'),
('https://unicode.example.com/test', 'Unicode: éñüñ 🚀 テスト', '2025-01-07 15:00:00+00', 'unicode-feed'),
('https://empty.example.com/title', '', '2025-01-08 16:00:00+00', 'empty-feed'),

-- 長いURL・タイトル用データ
('https://very-long-domain-name-for-testing-url-limits.example.com/very/long/path/to/article/with/many/segments', '非常に長いタイトルのテスト記事です。この記事は文字数制限やデータベースの制約をテストするために作られました。システムが長いテキストを適切に処理できることを確認します。', '2025-01-09 17:00:00+00', 'long-feed'),

-- limit テスト用データ（順序確認用）
('https://limit.test.com/item01', 'Limit テスト 01', '2025-02-01 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item02', 'Limit テスト 02', '2025-02-02 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item03', 'Limit テスト 03', '2025-02-03 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item04', 'Limit テスト 04', '2025-02-04 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item05', 'Limit テスト 05', '2025-02-05 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item06', 'Limit テスト 06', '2025-02-06 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item07', 'Limit テスト 07', '2025-02-07 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item08', 'Limit テスト 08', '2025-02-08 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item09', 'Limit テスト 09', '2025-02-09 09:00:00+00', 'limit-feed'),
('https://limit.test.com/item10', 'Limit テスト 10', '2025-02-10 09:00:00+00', 'limit-feed'),

-- 古い記事（日付フィルタリング用）
('https://old.example.com/article1', '古い記事1', '2024-01-01 10:00:00+00', 'old-feed'),
('https://old.example.com/article2', '古い記事2', '2024-12-31 23:59:59+00', 'old-feed');

-- articles テーブルのテストデータ
INSERT INTO articles (url, timestamp, status_code, content) VALUES
-- 成功ケース（status_code = 200）
('https://example.com/article1', '2025-01-01 10:30:00+00', 200, 'テスト記事1の内容です。'),
('https://example.com/article2', '2025-01-02 11:30:00+00', 200, 'テスト記事2の内容です。'),
('https://tech.example.com/rust-tutorial', '2025-01-10 14:30:00+00', 200, 'Rustの詳細なチュートリアル内容です。'),
('https://tech.example.com/python-guide', '2025-01-11 15:30:00+00', 200, 'Pythonガイドの詳細な内容です。'),
('https://news.example.com/breaking-news', '2025-01-12 16:30:00+00', 200, '重要なニュースの内容です。'),

-- エラーケース（各種status_code）
('https://example.com/article3', '2025-01-03 12:30:00+00', 404, ''),
('https://blog.different.org/post1', '2025-01-13 17:30:00+00', 403, 'アクセス拒否'),
('https://boundary.test.com/start', '2025-01-01 00:30:00+00', 500, 'サーバーエラー'),
('https://boundary.test.com/end', '2025-12-31 23:59:59+00', 503, 'サービス利用不可'),

-- 特殊文字・エッジケース
('https://special.example.com/chars', '2025-01-06 14:30:00+00', 200, 'Content with "quotes", <html>, & special chars: éñüñ'),
('https://unicode.example.com/test', '2025-01-07 15:30:00+00', 200, 'Unicode content: 🚀 éñüñ テスト内容'),
('https://empty.example.com/title', '2025-01-08 16:30:00+00', 200, ''),

-- 長いコンテンツ
('https://very-long-domain-name-for-testing-url-limits.example.com/very/long/path/to/article/with/many/segments', '2025-01-09 17:30:00+00', 200, '非常に長い内容のテスト記事です。この記事の内容は非常に長く、データベースの文字列制限やメモリ使用量をテストするために作られました。内容をもう少し長くするために追加のテキストを含めています。システムが大量のテキストデータを適切に処理できることを確認するためのテストデータです。さらに長くするために、Rustの特徴について説明します。Rustは安全性、速度、並行性に焦点を当てたプログラミング言語です。メモリ安全性を保証しながら、C/C++に匹敵する性能を実現しています。'),

-- limit テスト用データ（一部のみ成功）
('https://limit.test.com/item01', '2025-02-01 09:30:00+00', 200, 'Limit テスト用記事1の内容'),
('https://limit.test.com/item02', '2025-02-02 09:30:00+00', 200, 'Limit テスト用記事2の内容'),
('https://limit.test.com/item03', '2025-02-03 09:30:00+00', 404, ''),
('https://limit.test.com/item04', '2025-02-04 09:30:00+00', 200, 'Limit テスト用記事4の内容'),
('https://limit.test.com/item05', '2025-02-05 09:30:00+00', 500, 'サーバーエラー'),
('https://limit.test.com/item06', '2025-02-06 09:30:00+00', 200, 'Limit テスト用記事6の内容'),
('https://limit.test.com/item07', '2025-02-07 09:30:00+00', 200, 'Limit テスト用記事7の内容'),

-- 古い記事（一部のみ成功）
('https://old.example.com/article1', '2024-01-01 10:30:00+00', 200, '古い記事1の内容です。'),
('https://old.example.com/article2', '2024-12-31 23:59:59+00', 403, 'アクセス拒否');