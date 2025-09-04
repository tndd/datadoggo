-- 基本的な記事統合テスト用のデータ
-- test_get_articles_status 用

-- RSSリンクデータ
INSERT INTO rss_links (link, title, pub_date)
VALUES 
    (
        'https://test.com/link1',
        'テストリンク1', 
        CURRENT_TIMESTAMP
    ),
    (
        'https://test.com/link2',
        'テストリンク2',
        CURRENT_TIMESTAMP
    );

-- 記事データ（link1に対応する記事のみ）
INSERT INTO articles (url, status_code, content)
VALUES 
    (
        'https://test.com/link1',
        200,
        '記事内容1'
    );