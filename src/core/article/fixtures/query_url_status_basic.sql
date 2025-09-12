-- URL状態検索の基本テスト用データ
-- 目的: search_article_url_statuses関数の基本動作をテスト

-- article_linksテーブル: ソート検証のためのシンプルなデータ
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://example.com/article-a', 'Article A', '2025-01-01 10:00:00+00', 'test-feed'),
('https://example.com/article-b', 'Article B', '2025-01-02 11:00:00+00', 'test-feed'),
('https://example.org/post-c', 'Post C', '2025-01-03 12:00:00+00', 'other-feed');

-- articlesテーブル: 異なるステータスコードでのテスト
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://example.com/article-a', '2025-01-01 10:30:00+00', 200, 'Success content A'),
('https://example.com/article-b', '2025-01-02 11:30:00+00', 404, 'Not found content B');
-- post-c は未処理（articlesテーブルにレコードなし）