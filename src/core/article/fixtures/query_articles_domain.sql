-- ドメイン向け記事検索テスト用データ
-- 目的: search_articles関数の変換処理と成功ステータスのみ取得を検証

-- article_linksテーブル: 変換テスト用の基本データ
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://valid1.com/article', 'Valid Article 1', '2025-01-02 11:00:00+00', 'feed1'),
('https://valid2.com/article', 'Valid Article 2', '2025-01-01 10:00:00+00', 'feed1'),
('https://error.com/article', 'Error Article', '2025-01-03 12:00:00+00', 'feed2'),
('https://incomplete.com/article', 'Incomplete Article', '2025-01-04 13:00:00+00', 'feed3');

-- articlesテーブル: 様々なケースでの変換テスト
INSERT INTO articles (url, timestamp, status_code, content) VALUES
-- 正常なケース: status_code=200, contentあり, timestampあり
('https://valid1.com/article', '2025-01-02 11:30:00+00', 200, 'Complete valid content 1'),
('https://valid2.com/article', '2025-01-01 10:30:00+00', 200, 'Complete valid content 2'),
-- エラーケース: status_code!=200 → 除外される
('https://error.com/article', '2025-01-03 12:30:00+00', 404, 'Error content'),
-- 不完全ケース: status_code=200だが、contentが空 → 除外される
('https://incomplete.com/article', '2025-01-04 13:30:00+00', 200, '');