-- クエリフィルタリングテスト用のデータ
-- test_article_query_filters 用

-- RSSリンクデータ（複数のドメインとステータス）
INSERT INTO article_links (url, title, pub_date, source)
VALUES 
    -- example.comドメインの記事
    (
        'https://example.com/news1',
        'ニュース1',
        CURRENT_TIMESTAMP,
        'test'
    ),
    (
        'https://example.com/news2', 
        'ニュース2',
        CURRENT_TIMESTAMP,
        'test'
    ),
    -- different.comドメインの記事  
    (
        'https://different.com/news3',
        'ニュース3',
        CURRENT_TIMESTAMP,
        'test'
    );

-- 記事データ（異なるステータスコード）
INSERT INTO articles (url, status_code, content)
VALUES 
    -- 成功記事
    (
        'https://example.com/news1',
        200,
        'ニュース1の内容'
    ),
    -- エラー記事
    (
        'https://example.com/news2',
        404,
        'エラー内容'
    ),
    -- 別ドメインの成功記事
    (
        'https://different.com/news3',
        200,
        'ニュース3の内容'
    );