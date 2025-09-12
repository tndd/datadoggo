-- 境界値・エッジケーステスト用データ

-- article_links テーブルの境界値データ
INSERT INTO article_links (url, title, pub_date, source) VALUES
-- 日付境界値
('https://boundary.test.com/start', '境界値: 年始', '2025-01-01 00:00:00+00', 'boundary-feed'),
('https://boundary.test.com/end', '境界値: 年末', '2025-12-31 23:59:59+00', 'boundary-feed'),
('https://boundary.test.com/middle', '境界値: 中間', '2025-06-15 12:00:00+00', 'boundary-feed'),

-- 特殊文字・エッジケース
('https://special.example.com/chars', 'Title with "quotes" & <tags>', '2025-01-06 14:00:00+00', 'special-feed'),
('https://unicode.example.com/test', 'Unicode: éñüñ 🚀 テスト', '2025-01-07 15:00:00+00', 'unicode-feed'),
('https://empty.example.com/title', '', '2025-01-08 16:00:00+00', 'empty-feed'),

-- 長いURL・タイトル
('https://very-long-domain-name-for-testing-url-limits.example.com/very/long/path/to/article/with/many/segments', '非常に長いタイトルのテスト記事です。この記事は文字数制限やデータベースの制約をテストするために作られました。', '2025-01-09 17:00:00+00', 'long-feed'),

-- 古い記事（日付フィルタリング用）
('https://old.example.com/article1', '古い記事1', '2024-01-01 10:00:00+00', 'old-feed'),
('https://old.example.com/article2', '古い記事2', '2024-12-31 23:59:59+00', 'old-feed');

-- articles テーブルの境界値データ
INSERT INTO articles (url, timestamp, status_code, content) VALUES
-- 境界値日付・エラーケース
('https://boundary.test.com/start', '2025-01-01 00:30:00+00', 500, 'サーバーエラー'),
('https://boundary.test.com/end', '2025-12-31 23:59:59+00', 503, 'サービス利用不可'),
('https://boundary.test.com/middle', '2025-06-15 12:30:00+00', 200, '境界値テスト: 中間の記事'),

-- 特殊文字・エッジケース
('https://special.example.com/chars', '2025-01-06 14:30:00+00', 200, 'Content with "quotes", <html>, & special chars: éñüñ'),
('https://unicode.example.com/test', '2025-01-07 15:30:00+00', 200, 'Unicode content: 🚀 éñüñ テスト内容'),
('https://empty.example.com/title', '2025-01-08 16:30:00+00', 200, ''),

-- 長いコンテンツ
('https://very-long-domain-name-for-testing-url-limits.example.com/very/long/path/to/article/with/many/segments', '2025-01-09 17:30:00+00', 200, '非常に長い内容のテスト記事です。この記事の内容は非常に長く、データベースの文字列制限やメモリ使用量をテストするために作られました。内容をもう少し長くするために追加のテキストを含めています。システムが大量のテキストデータを適切に処理できることを確認するためのテストデータです。さらに長くするために、Rustの特徴について説明します。Rustは安全性、速度、並行性に焦点を当てたプログラミング言語です。メモリ安全性を保証しながら、C/C++に匹敵する性能を実現しています。'),

-- 古い記事
('https://old.example.com/article1', '2024-01-01 10:30:00+00', 200, '古い記事1の内容です。'),
('https://old.example.com/article2', '2024-12-31 23:59:59+00', 403, 'アクセス拒否');