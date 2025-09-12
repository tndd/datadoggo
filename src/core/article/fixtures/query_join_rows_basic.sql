-- 記事結合検索の基本テスト用データ
-- 目的: search_article_join_rows関数の基本動作とソート検証

-- article_linksテーブル: 日付降順ソート検証用
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://latest.com/article', 'Latest Article', '2025-01-03 12:00:00+00', 'news'),
('https://middle.com/article', 'Middle Article', '2025-01-02 11:00:00+00', 'blog'),
('https://oldest.com/article', 'Oldest Article', '2025-01-01 10:00:00+00', 'tech');

-- articlesテーブル: 結合データの整合性検証用
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://latest.com/article', '2025-01-03 12:30:00+00', 200, 'Latest content with timestamp'),
('https://middle.com/article', '2025-01-02 11:30:00+00', 404, 'Middle content with error'),
('https://oldest.com/article', '2025-01-01 10:30:00+00', 200, 'Oldest content with timestamp');