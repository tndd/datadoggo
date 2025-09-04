-- バックログ記事テスト用のデータ  
-- test_search_backlog_articles_light 用

-- RSSリンクデータ
INSERT INTO article_links (link, title, pub_date, source)
VALUES 
    -- 未処理リンク
    (
        'https://test.com/trait_unprocessed',
        'トレイト未処理リンク',
        CURRENT_TIMESTAMP,
        'test'
    ),
    -- エラーリンク
    (
        'https://test.com/trait_error',
        'トレイトエラーリンク', 
        CURRENT_TIMESTAMP,
        'test'
    ),
    -- 成功リンク
    (
        'https://test.com/trait_success',
        'トレイト成功リンク',
        CURRENT_TIMESTAMP,
        'test'
    );

-- 記事データ（エラーと成功のみ）
INSERT INTO articles (url, status_code, content)
VALUES 
    -- エラー記事
    (
        'https://test.com/trait_error',
        500,
        'トレイトエラー記事内容'
    ),
    -- 成功記事
    (
        'https://test.com/trait_success',
        200,
        'トレイト成功記事内容'
    );