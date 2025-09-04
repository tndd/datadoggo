-- workflow成功/エラー混在テスト用fixture
-- 様々な状況を想定した現実的なテストデータ

-- RSSリンクデータ（12件）
INSERT INTO article_links (link, title, pub_date, source) VALUES 
    -- 成功予定のURL（6件）
    ('https://success.example.com/news1', '成功記事1', '2025-01-01 09:00:00+00:00', 'test'),
    ('https://success.example.com/news2', '成功記事2', '2025-01-01 10:00:00+00:00', 'test'),
    ('https://reliable.site.com/article1', '信頼できる記事1', '2025-01-01 11:00:00+00:00', 'test'),
    ('https://reliable.site.com/article2', '信頼できる記事2', '2025-01-01 12:00:00+00:00', 'test'),
    ('https://good.blog.net/post1', '良質なブログ記事', '2025-01-01 13:00:00+00:00', 'test'),
    ('https://working.news.org/update', '動作する更新', '2025-01-01 14:00:00+00:00', 'test'),
    
    -- エラー発生予定のURL（4件）
    ('https://error.example.com/broken', '破損記事', '2025-01-01 15:00:00+00:00', 'test'),
    ('https://timeout.slow.com/article', 'タイムアウト記事', '2025-01-01 16:00:00+00:00', 'test'),
    ('https://forbidden.site.com/secret', '禁止記事', '2025-01-01 17:00:00+00:00', 'test'),
    ('https://notfound.example.com/missing', '見つからない記事', '2025-01-01 18:00:00+00:00', 'test'),
    
    -- 既に処理済みのURL（2件）
    ('https://old.processed.com/done1', '既処理記事1', '2024-12-20 10:00:00+00:00', 'test'),
    ('https://old.processed.com/done2', '既処理記事2', '2024-12-20 11:00:00+00:00', 'test');

-- 既に処理済みの記事データ（2件）
INSERT INTO articles (url, timestamp, status_code, content) VALUES 
    ('https://old.processed.com/done1', '2024-12-20 10:30:00+00:00', 200, '既に処理済みの記事1の内容です。'),
    ('https://old.processed.com/done2', '2024-12-20 11:30:00+00:00', 500, '取得エラー: 既に処理済みでエラーになった記事2です');