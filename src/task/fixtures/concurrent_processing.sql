-- 並行処理機能テスト用fixture
-- 複数の処理が同時に実行された場合の動作確認用データ

-- 複数のフィード用記事リンク（異なるsourceで区別）
INSERT INTO article_links (url, title, pub_date, source) VALUES
-- フィード1: 技術ニュース
('https://tech-feed1.example.com/article-1', '技術記事1', '2025-01-20T10:00:00Z', 'tech-feed-1'),
('https://tech-feed1.example.com/article-2', '技術記事2', '2025-01-20T10:30:00Z', 'tech-feed-1'),
('https://tech-feed1.example.com/article-3', '技術記事3', '2025-01-20T11:00:00Z', 'tech-feed-1'),

-- フィード2: ビジネスニュース
('https://business-feed2.example.com/news-1', 'ビジネス記事1', '2025-01-20T10:15:00Z', 'business-feed-2'),
('https://business-feed2.example.com/news-2', 'ビジネス記事2', '2025-01-20T10:45:00Z', 'business-feed-2'),
('https://business-feed2.example.com/news-3', 'ビジネス記事3', '2025-01-20T11:15:00Z', 'business-feed-2'),

-- フィード3: エンターテイメント
('https://entertainment-feed3.example.com/story-1', 'エンタメ記事1', '2025-01-20T10:10:00Z', 'entertainment-feed-3'),
('https://entertainment-feed3.example.com/story-2', 'エンタメ記事2', '2025-01-20T10:40:00Z', 'entertainment-feed-3'),
('https://entertainment-feed3.example.com/story-3', 'エンタメ記事3', '2025-01-20T11:10:00Z', 'entertainment-feed-3');

-- 競合状態テスト用：単一の記事（重複はテスト内で作成）
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://shared-article-concurrent.example.com/important-news', 'フィード1からの重要ニュース', '2025-01-20T12:00:00Z', 'feed-source-1');

-- 記事処理状態の混在（並行処理で部分的に処理済みの状況をシミュレート）
-- 一部の記事は既に処理済み
INSERT INTO articles (url, timestamp, status_code, content) VALUES
-- 成功済み記事（再処理されない）
('https://tech-feed1.example.com/article-1', '2025-01-20T10:30:00Z', 200, '既に処理済みの技術記事1'),
('https://business-feed2.example.com/news-1', '2025-01-20T10:45:00Z', 200, '既に処理済みのビジネス記事1'),

-- エラー記事（再処理対象）
('https://tech-feed1.example.com/article-2', '2025-01-20T11:00:00Z', 500, '前回処理でエラー'),
('https://entertainment-feed3.example.com/story-1', '2025-01-20T10:40:00Z', 404, '見つからなかったエンタメ記事');

-- バッチ処理境界のテスト用
-- search_backlog_article_links の LIMIT 100 境界を意識したデータ
INSERT INTO article_links (url, title, pub_date, source) VALUES
-- 大量の未処理記事（実際の境界テスト用）
('https://batch-test.example.com/unprocessed-01', '未処理記事01', '2025-01-19T09:00:00Z', 'batch-test'),
('https://batch-test.example.com/unprocessed-02', '未処理記事02', '2025-01-19T09:01:00Z', 'batch-test'),
('https://batch-test.example.com/unprocessed-03', '未処理記事03', '2025-01-19T09:02:00Z', 'batch-test'),
('https://batch-test.example.com/unprocessed-04', '未処理記事04', '2025-01-19T09:03:00Z', 'batch-test'),
('https://batch-test.example.com/unprocessed-05', '未処理記事05', '2025-01-19T09:04:00Z', 'batch-test'),
('https://batch-test.example.com/unprocessed-06', '未処理記事06', '2025-01-19T09:05:00Z', 'batch-test'),
('https://batch-test.example.com/unprocessed-07', '未処理記事07', '2025-01-19T09:06:00Z', 'batch-test'),
('https://batch-test.example.com/unprocessed-08', '未処理記事08', '2025-01-19T09:07:00Z', 'batch-test'),
('https://batch-test.example.com/unprocessed-09', '未処理記事09', '2025-01-19T09:08:00Z', 'batch-test'),
('https://batch-test.example.com/unprocessed-10', '未処理記事10', '2025-01-19T09:09:00Z', 'batch-test');

-- タイムスタンプ競合テスト用：処理順序の確認
INSERT INTO article_links (url, title, pub_date, source) VALUES
-- 同じ記事だが処理タイミングが異なる場合の動作確認
('https://timestamp-race.example.com/article', '時刻競合テスト記事', '2025-01-20T13:00:00Z', 'timestamp-test-1');

-- 対応する古い記事データ（更新されるべき）
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://timestamp-race.example.com/article', '2025-01-20T12:00:00Z', 200, '古い記事内容（更新されるべき）');

-- エラー回復シナリオ用：一部成功、一部失敗の混在状況
INSERT INTO article_links (url, title, pub_date, source) VALUES
-- 混在シナリオ記事
('https://mixed-result.example.com/success-1', '成功予定記事1', '2025-01-20T14:00:00Z', 'mixed-test'),
('https://mixed-result.example.com/success-2', '成功予定記事2', '2025-01-20T14:01:00Z', 'mixed-test'),
('https://mixed-result.example.com/fail-1', '失敗予定記事1', '2025-01-20T14:02:00Z', 'mixed-test'),
('https://mixed-result.example.com/fail-2', '失敗予定記事2', '2025-01-20T14:03:00Z', 'mixed-test');

-- バルク処理でのUPSERT動作確認用：既存データとの競合
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://upsert-test.example.com/update-me', '更新されるタイトル', '2025-01-20T15:00:00Z', 'upsert-test');

-- 対応する既存記事
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://upsert-test.example.com/update-me', '2025-01-20T14:30:00Z', 200, '古い内容（UPSERTで更新されるべき）');