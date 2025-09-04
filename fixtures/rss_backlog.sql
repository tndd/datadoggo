-- search_unprocessed_article_links テスト用のフィクスチャ
-- 未処理またはエラーの記事リンクを取得するテスト用データ

-- 記事リンクデータ
INSERT INTO article_links (link, title, pub_date, source)
VALUES 
    -- 未処理リンク（articlesテーブルに対応するレコードなし）
    (
        'https://example.com/unprocessed-article-1',
        '未処理記事1',
        '2025-09-04T10:00:00Z',
        'test'
    ),
    (
        'https://example.com/unprocessed-article-2',
        '未処理記事2',
        '2025-09-04T09:00:00Z',
        'test'
    ),
    -- エラー状態のリンク（status_code != 200）
    (
        'https://example.com/error-article-1',
        'エラー記事1',
        '2025-09-04T08:00:00Z',
        'test'
    ),
    (
        'https://example.com/error-article-2',
        'エラー記事2',
        '2025-09-04T07:00:00Z',
        'test'
    ),
    -- 正常処理済みリンク（status_code = 200）
    (
        'https://example.com/success-article-1',
        '正常記事1',
        '2025-09-04T06:00:00Z',
        'test'
    ),
    (
        'https://example.com/success-article-2',
        '正常記事2',
        '2025-09-04T05:00:00Z',
        'test'
    ),
    -- 複数エラーパターン
    (
        'https://example.com/timeout-article',
        'タイムアウト記事',
        '2025-09-04T04:00:00Z',
        'test'
    ),
    (
        'https://example.com/notfound-article',
        '404記事',
        '2025-09-04T03:00:00Z',
        'test'
    );

-- 記事データ（エラーと正常のみ）
INSERT INTO articles (url, status_code, content)
VALUES 
    -- エラー記事（status_code != 200）
    (
        'https://example.com/error-article-1',
        500,
        'サーバーエラー'
    ),
    (
        'https://example.com/error-article-2',
        503,
        'サービス利用不可'
    ),
    (
        'https://example.com/timeout-article',
        408,
        'リクエストタイムアウト'
    ),
    (
        'https://example.com/notfound-article',
        404,
        'ページが見つかりません'
    ),
    -- 正常記事（status_code = 200）
    (
        'https://example.com/success-article-1',
        200,
        '正常な記事内容1'
    ),
    (
        'https://example.com/success-article-2',
        200,
        '正常な記事内容2'
    );