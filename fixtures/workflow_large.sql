-- workflow大規模処理テスト用fixture
-- 25件のrss_linksで大量データ処理をテスト

-- RSSリンクデータ（25件）
INSERT INTO rss_links (link, title, pub_date) VALUES 
    -- 新規処理対象のURL（20件）
    ('https://large1.example.com/article1', '大規模テスト記事1', '2025-01-01 08:00:00+00:00'),
    ('https://large1.example.com/article2', '大規模テスト記事2', '2025-01-01 08:30:00+00:00'),
    ('https://large1.example.com/article3', '大規模テスト記事3', '2025-01-01 09:00:00+00:00'),
    ('https://large2.example.com/news1', '大規模ニュース1', '2025-01-01 09:30:00+00:00'),
    ('https://large2.example.com/news2', '大規模ニュース2', '2025-01-01 10:00:00+00:00'),
    ('https://large2.example.com/news3', '大規模ニュース3', '2025-01-01 10:30:00+00:00'),
    ('https://tech.large.com/update1', '技術アップデート1', '2025-01-01 11:00:00+00:00'),
    ('https://tech.large.com/update2', '技術アップデート2', '2025-01-01 11:30:00+00:00'),
    ('https://tech.large.com/update3', '技術アップデート3', '2025-01-01 12:00:00+00:00'),
    ('https://blog.massive.org/post1', '大量ブログ記事1', '2025-01-01 12:30:00+00:00'),
    ('https://blog.massive.org/post2', '大量ブログ記事2', '2025-01-01 13:00:00+00:00'),
    ('https://blog.massive.org/post3', '大量ブログ記事3', '2025-01-01 13:30:00+00:00'),
    ('https://error.large.com/broken1', 'エラー記事1', '2025-01-01 14:00:00+00:00'),
    ('https://error.large.com/broken2', 'エラー記事2', '2025-01-01 14:30:00+00:00'),
    ('https://timeout.large.com/slow1', 'スロー記事1', '2025-01-01 15:00:00+00:00'),
    ('https://timeout.large.com/slow2', 'スロー記事2', '2025-01-01 15:30:00+00:00'),
    ('https://mixed.large.com/good1', '混在記事良1', '2025-01-01 16:00:00+00:00'),
    ('https://mixed.large.com/good2', '混在記事良2', '2025-01-01 16:30:00+00:00'),
    ('https://mixed.large.com/bad1', '混在記事悪1', '2025-01-01 17:00:00+00:00'),
    ('https://mixed.large.com/bad2', '混在記事悪2', '2025-01-01 17:30:00+00:00'),
    
    -- 既に処理済みのURL（5件）
    ('https://old.large.com/processed1', '大規模既処理1', '2024-12-15 10:00:00+00:00'),
    ('https://old.large.com/processed2', '大規模既処理2', '2024-12-15 11:00:00+00:00'),
    ('https://old.large.com/processed3', '大規模既処理3', '2024-12-15 12:00:00+00:00'),
    ('https://archive.large.com/old1', '大規模アーカイブ1', '2024-12-10 10:00:00+00:00'),
    ('https://archive.large.com/old2', '大規模アーカイブ2', '2024-12-10 11:00:00+00:00');

-- 既に処理済みの記事データ（5件）
INSERT INTO articles (url, timestamp, status_code, content) VALUES 
    ('https://old.large.com/processed1', '2024-12-15 10:30:00+00:00', 200, '大規模既処理記事1の内容です。'),
    ('https://old.large.com/processed2', '2024-12-15 11:30:00+00:00', 200, '大規模既処理記事2の内容です。'),
    ('https://old.large.com/processed3', '2024-12-15 12:30:00+00:00', 500, '取得エラー: 大規模既処理記事3はエラーでした'),
    ('https://archive.large.com/old1', '2024-12-10 10:30:00+00:00', 200, '大規模アーカイブ記事1の内容です。'),
    ('https://archive.large.com/old2', '2024-12-10 11:30:00+00:00', 500, '取得エラー: 大規模アーカイブ記事2はアクセス不可でした');