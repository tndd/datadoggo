-- RSS記事のテストデータ（RssLink構造対応：link, title, pub_dateのみ）
INSERT INTO rss_links (link, title, pub_date)
VALUES -- 基本テストデータ（save_testsで重複テスト用として使用）
    (
        'https://test.example.com/article1',
        'テスト記事1',
        '2025-08-26T10:00:00Z'
    ),
    (
        'https://test.example.com/article2',
        'テスト記事2',
        '2025-08-26T11:00:00Z'
    ),
    (
        'https://news.sample.org/breaking',
        'ニュース記事',
        '2025-08-26T09:30:00Z'
    ),
    (
        'https://blog.tech.net/update',
        '技術ブログ',
        '2025-08-26T12:15:00Z'
    ),
    -- 日付境界テスト用（厳密な境界値）
    (
        'https://test.com/boundary/exactly-start',
        'Boundary Start Article',
        '2025-01-15T00:00:00Z'
    ),
    (
        'https://test.com/boundary/exactly-end',
        'Boundary End Article',
        '2025-01-15T23:59:59Z'
    ),
    (
        'https://test.com/boundary/one-second-before',
        'Just Before Range',
        '2025-01-14T23:59:59Z'
    ),
    (
        'https://test.com/boundary/one-second-after',
        'Just After Range',
        '2025-01-16T00:00:01Z'
    ),
    -- 取得機能テスト用
    (
        'https://example.com/tech/article-2025-01-15',
        'Tech News 2025',
        '2025-01-15T10:00:00Z'
    ),
    (
        'https://news.example.com/world/breaking-news',
        'Breaking World News',
        '2025-01-14T15:30:00Z'
    ),
    (
        'https://blog.example.com/lifestyle/health-tips',
        'Health Tips for 2025',
        '2025-01-16T08:45:00Z'
    ),
    -- URL部分一致テスト用
    (
        'https://example.com/test',
        'Simple Test',
        '2025-01-10T12:00:00Z'
    ),
    (
        'https://not-example.com/test',
        'Not Example Test',
        '2025-01-11T12:00:00Z'
    ),
    -- 特殊文字・エスケープテスト用
    (
        'https://special.com/article%20with%20spaces',
        'Article With Spaces',
        '2025-01-13T12:00:00Z'
    ),
    (
        'https://special.com/article_with_underscore',
        'Article With Underscore',
        '2025-01-13T13:00:00Z'
    ),
    -- 大小文字混合テスト用（ILIKE確認）
    (
        'https://CaseSensitive.com/MixedCase',
        'Mixed Case Article',
        '2025-01-14T12:00:00Z'
    ),
    -- シンプル記事（pub_date必須のため残す）
    (
        'https://minimal.site.com/simple',
        'シンプル記事',
        '2025-08-26T13:45:00Z'
    );
-- 注意：pub_dateがNULLのレコードは新しいスキーマでは対応不可のため削除