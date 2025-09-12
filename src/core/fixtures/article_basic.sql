-- NOTE: src/core/article.rs からの #[sqlx::test(fixtures("article_basic"))] 向け
-- テスト用のarticle_linksデータ
INSERT INTO article_links (url, title, pub_date, source) VALUES
('https://example.com/article1', 'テスト記事1', '2025-01-01 10:00:00+00', 'test-feed'),
('https://example.com/article2', 'テスト記事2', '2025-01-02 11:00:00+00', 'test-feed'),
('https://example.com/article3', 'テスト記事3', '2025-01-03 12:00:00+00', 'other-feed'),
('https://example.com/article4', '古い記事', '2024-12-31 09:00:00+00', 'test-feed'),
('https://different.com/article5', '別ドメインの記事', '2025-01-05 13:00:00+00', 'test-feed'),
-- エッジケース用データ
('https://example.com/special-chars', 'Title with "quotes" & <tags>', '2025-01-06 14:00:00+00', 'test-feed'),
('https://example.com/empty-title', '', '2025-01-07 15:00:00+00', 'test-feed'),
('https://very-long-domain-name-for-testing-url-limits.example.com/very/long/path/to/article', '非常に長いタイトルのテスト記事です。この記事は文字数制限やデータベースの制約をテストするために作られました。', '2025-01-08 16:00:00+00', 'test-feed');

-- テスト用のarticlesデータ（成功・失敗混在）
INSERT INTO articles (url, timestamp, status_code, content) VALUES
('https://example.com/article1', '2025-01-01 10:30:00+00', 200, 'テスト記事1の内容です。'),
('https://example.com/article2', '2025-01-02 11:30:00+00', 200, 'テスト記事2の内容です。'),
('https://example.com/article3', '2025-01-03 12:30:00+00', 404, ''),
('https://example.com/article4', '2024-12-31 09:30:00+00', 200, '古い記事の内容です。'),
('https://different.com/article5', '2025-01-05 13:30:00+00', 500, ''),
-- エッジケース用データ
('https://example.com/special-chars', '2025-01-06 14:30:00+00', 200, 'Content with "quotes", <html>, & special chars: éñüñ'),
('https://example.com/empty-title', '2025-01-07 15:30:00+00', 200, ''),
('https://very-long-domain-name-for-testing-url-limits.example.com/very/long/path/to/article', '2025-01-08 16:30:00+00', 200, '非常に長い内容のテスト記事です。この記事の内容は非常に長く、データベースの文字列制限やメモリ使用量をテストするために作られました。内容をもう少し長くするために追加のテキストを含めています。システムが大量のテキストデータを適切に処理できることを確認するためのテストデータです。');
