-- エラー回復シナリオ機能テスト用fixture
-- 様々なエラー状況からの適切な回復処理を検証

-- 段階的回復テスト用：異なるエラー状況の記事リンク
INSERT INTO article_links (url, title, pub_date, source) VALUES
-- 新規未処理記事（初回処理対象）
('https://recovery-test.example.com/new-article-1', '新規記事1', '2025-01-21T10:00:00Z', 'recovery-test'),
('https://recovery-test.example.com/new-article-2', '新規記事2', '2025-01-21T10:01:00Z', 'recovery-test'),
('https://recovery-test.example.com/new-article-3', '新規記事3', '2025-01-21T10:02:00Z', 'recovery-test'),

-- 過去にエラーだった記事（再処理対象）
('https://recovery-test.example.com/retry-500-error', '500エラー再試行記事', '2025-01-21T09:00:00Z', 'recovery-test'),
('https://recovery-test.example.com/retry-404-error', '404エラー再試行記事', '2025-01-21T09:01:00Z', 'recovery-test'),
('https://recovery-test.example.com/retry-timeout', 'タイムアウト再試行記事', '2025-01-21T09:02:00Z', 'recovery-test'),

-- 成功記事（再処理対象外）
('https://recovery-test.example.com/already-success', '既に成功した記事', '2025-01-21T08:00:00Z', 'recovery-test');

-- 対応するarticlesテーブルのエラー記事データ
INSERT INTO articles (url, timestamp, status_code, content) VALUES
-- サーバーエラー（500）記事
('https://recovery-test.example.com/retry-500-error', '2025-01-21T09:30:00Z', 500, '記事取得APIエラー: サーバー内部エラー'),
('https://recovery-test.example.com/retry-404-error', '2025-01-21T09:31:00Z', 404, '記事取得APIエラー: ページが見つかりません'),
('https://recovery-test.example.com/retry-timeout', '2025-01-21T09:32:00Z', 500, '記事取得APIエラー: タイムアウト'),

-- 成功記事（status_code=200、再処理されない）
('https://recovery-test.example.com/already-success', '2025-01-21T08:30:00Z', 200, '既に取得済みの成功記事内容');

-- 部分的エラー回復シナリオ用：混在状況
INSERT INTO article_links (url, title, pub_date, source) VALUES
-- エラーのパターンバリエーション
('https://mixed-errors.example.com/client-error-401', '認証エラー記事', '2025-01-21T11:00:00Z', 'error-patterns'),
('https://mixed-errors.example.com/client-error-403', 'アクセス禁止記事', '2025-01-21T11:01:00Z', 'error-patterns'),
('https://mixed-errors.example.com/server-error-502', 'バッドゲートウェイ記事', '2025-01-21T11:02:00Z', 'error-patterns'),
('https://mixed-errors.example.com/server-error-503', 'サービス利用不可記事', '2025-01-21T11:03:00Z', 'error-patterns');

-- 対応するエラー記事
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://mixed-errors.example.com/client-error-401', '2025-01-21T11:30:00Z', 401, '認証に失敗しました'),
('https://mixed-errors.example.com/client-error-403', '2025-01-21T11:31:00Z', 403, 'アクセスが拒否されました'),
('https://mixed-errors.example.com/server-error-502', '2025-01-21T11:32:00Z', 502, 'バッドゲートウェイエラー'),
('https://mixed-errors.example.com/server-error-503', '2025-01-21T11:33:00Z', 503, 'サービス利用不可');

-- エラー回復の継続処理テスト用
-- task_collect_articles が途中で失敗しても残りの記事を処理し続けることを確認
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://continuity-test.example.com/process-1', '継続処理テスト1', '2025-01-21T12:00:00Z', 'continuity'),
('https://continuity-test.example.com/process-2', '継続処理テスト2', '2025-01-21T12:01:00Z', 'continuity'),
('https://continuity-test.example.com/process-3', '継続処理テスト3', '2025-01-21T12:02:00Z', 'continuity'),
('https://continuity-test.example.com/process-4', '継続処理テスト4', '2025-01-21T12:03:00Z', 'continuity'),
('https://continuity-test.example.com/process-5', '継続処理テスト5', '2025-01-21T12:04:00Z', 'continuity');

-- 重複URL のエラー回復：同じURLで複数のエラー記録がある場合
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://duplicate-error-recovery.example.com/same-url', '重複エラー記事', '2025-01-21T13:00:00Z', 'duplicate-test');

-- 単一のエラー記録（複数の時点での記録はUPSERTで1つになる）
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://duplicate-error-recovery.example.com/same-url', '2025-01-21T13:30:00Z', 404, '2回目のエラー記録（最新）');

-- ネットワーク障害からの回復シナリオ
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://network-recovery.example.com/connection-failed', 'ネットワーク障害記事', '2025-01-21T14:00:00Z', 'network-test'),
('https://network-recovery.example.com/dns-failed', 'DNS解決失敗記事', '2025-01-21T14:01:00Z', 'network-test'),
('https://network-recovery.example.com/ssl-failed', 'SSL接続失敗記事', '2025-01-21T14:02:00Z', 'network-test');

-- ネットワーク関連エラーの記録
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://network-recovery.example.com/connection-failed', '2025-01-21T14:30:00Z', 500, '取得エラー: 接続に失敗しました'),
('https://network-recovery.example.com/dns-failed', '2025-01-21T14:31:00Z', 500, '取得エラー: DNS解決に失敗'),
('https://network-recovery.example.com/ssl-failed', '2025-01-21T14:32:00Z', 500, '取得エラー: SSL証明書エラー');

-- バッチ境界での回復処理：LIMIT 100 の境界近辺
INSERT INTO article_links (url, title, pub_date, source) VALUES
-- 日付の古い順で96-105番目の記事（LIMIT 100の境界テスト）
('https://batch-boundary.example.com/article-96', 'バッチ境界記事96', '2025-01-20T23:56:00Z', 'batch-boundary'),
('https://batch-boundary.example.com/article-97', 'バッチ境界記事97', '2025-01-20T23:57:00Z', 'batch-boundary'),
('https://batch-boundary.example.com/article-98', 'バッチ境界記事98', '2025-01-20T23:58:00Z', 'batch-boundary'),
('https://batch-boundary.example.com/article-99', 'バッチ境界記事99', '2025-01-20T23:59:00Z', 'batch-boundary'),
('https://batch-boundary.example.com/article-100', 'バッチ境界記事100', '2025-01-21T00:00:00Z', 'batch-boundary'),
('https://batch-boundary.example.com/article-101', 'バッチ境界記事101', '2025-01-21T00:01:00Z', 'batch-boundary'),
('https://batch-boundary.example.com/article-102', 'バッチ境界記事102', '2025-01-21T00:02:00Z', 'batch-boundary'),
('https://batch-boundary.example.com/article-103', 'バッチ境界記事103', '2025-01-21T00:03:00Z', 'batch-boundary'),
('https://batch-boundary.example.com/article-104', 'バッチ境界記事104', '2025-01-21T00:04:00Z', 'batch-boundary'),
('https://batch-boundary.example.com/article-105', 'バッチ境界記事105', '2025-01-21T00:05:00Z', 'batch-boundary');

-- 一部を既存エラー記事として登録（再処理対象）
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://batch-boundary.example.com/article-98', '2025-01-21T00:28:00Z', 500, 'バッチ境界エラー98'),
('https://batch-boundary.example.com/article-99', '2025-01-21T00:29:00Z', 404, 'バッチ境界エラー99'),
('https://batch-boundary.example.com/article-102', '2025-01-21T00:32:00Z', 500, 'バッチ境界エラー102');