-- 未処理リンクテスト用のデータ
-- test_search_unprocessed_rss_links 用

-- RSSリンクデータ
INSERT INTO rss_links (link, title, pub_date)  
VALUES
    -- 未処理リンク
    (
        'https://test.com/unprocessed',
        '未処理リンク',
        CURRENT_TIMESTAMP
    ),
    -- 処理済みリンク
    (
        'https://test.com/processed',
        '処理済みリンク',
        CURRENT_TIMESTAMP
    );

-- 記事データ（処理済みのみ）
INSERT INTO articles (url, status_code, content)
VALUES
    (
        'https://test.com/processed', 
        200,
        '処理済み記事内容'
    );