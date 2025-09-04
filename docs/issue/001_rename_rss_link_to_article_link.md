# `rss_link` を用途ベースの名前に改名し、リンクの出自を示す `source` を追加する

- 種別: リファクタリング / データベース変更（互換性破壊あり）
- 優先度: 高
- 影響範囲: DBスキーマ, ドメイン層, アプリ層ワークフロー, テスト, フィクスチャ, ドキュメント

## 背景 / 課題

`rss_link` は「RSSから抽出したリンク」という印象を与える名前だが、実態は「スクレイプ前の待機リンク（バックログ）のアンカー」であり、情報源がRSSに限定されない。用途に対して名称の抽象度が低く、将来の拡張（サイトマップ・キュレーションAPI・手動投入など）において不適切。

また、リンクの出自を保存するフィールド（例: `rss`, `sitemap`, `curation`, `manual` 等）がないため、由来別の集計やデバッグが困難。

## 現状（調査メモ）

- DB: `migrations/001_create_rss_links.sql` にて `rss_links` テーブル
- コード: `src/domain/rss.rs`
  - 構造体 `RssLink`
  - `get_rss_links_from_channel` / `get_rss_links_from_feed` / `store_rss_links` / `search_rss_links` / `search_unprocessed_rss_links`
- 参照箇所（一部）
  - `src/app/workflow.rs`: 取得・保存・未処理検索を呼び出し
  - `src/domain/article.rs`: `LEFT JOIN articles a ON rl.link = a.url` など `rss_links` 前提のクエリ
  - `fixtures/rss.sql`, `fixtures/workflow.sql`, `fixtures/workflow_mixed.sql`: `rss_links` への INSERT
  - `README.md`: システム概要に `rss_link` の説明
  - `migrations/002_create_articles.sql`: コメントで `rss_links.link` への言及

## 目標

- 名称を用途ベースへ変更し、ソース非依存のモデルにする
  - テーブル: `rss_links` → `article_links`
  - 構造体: `RssLink` → `ArticleLink`
  - 関数名・クエリ名から `rss` 固有の語を排し抽象化（ただしRSSパース自体はRSS専用モジュールに閉じ込める）
- リンクの由来を示す `source` フィールドを追加

## 提案 / 設計

1) DBスキーマ移行
- テーブル名変更: `ALTER TABLE rss_links RENAME TO article_links;`
- 列追加: `ALTER TABLE article_links ADD COLUMN source TEXT NOT NULL DEFAULT 'rss';`
  - 将来的にENUM化も検討（`rss`, `sitemap`, `curation`, `manual` 等）
- 既存データは `source='rss'` として移行

2) コード分割・命名整理
- 汎用リンク層を新設（例: `src/domain/link.rs`）し、`ArticleLink` と保存・検索APIを集約
- RSSパーサは `src/domain/rss.rs` に残し、返り値を `ArticleLink` に統一
  - `get_rss_links_from_channel` → `get_article_links_from_channel`（返り値型を変更）
  - 保存は新API `store_article_links` を使用
- 既存APIの移行マッピング
  - `RssLink` → `ArticleLink`
  - `store_rss_links` → `store_article_links`
  - `search_rss_links` → `search_article_links`
  - `search_unprocessed_rss_links` → `search_unprocessed_article_links`
- `src/app/workflow.rs` と `src/domain/article.rs` のJOIN対象テーブルを `article_links` に変更

3) テスト / フィクスチャ更新
- `#[sqlx::test(fixtures("../../fixtures/rss.sql"))]` 内の INSERT 先を `article_links` に変更
- テスト内シンボル名・メッセージを新名称に合わせて更新

4) ドキュメント更新
- `README.md` のフロー説明（`rss_link` → `article_link` と `source` の説明）
- `migrations/002_create_articles.sql` などのコメントを現状に合わせて修正

## マイグレーション計画

- 新規マイグレーション `003_rename_rss_links_to_article_links.sql` を追加
  - RENAME と `source` 列追加、必要ならインデックス再作成
- コード・テスト・フィクスチャを同一PRで一括更新
  - 段階移行が必要な場合は一時的にDB View `rss_links` を作る選択肢もあるが、基本は不要想定

## タスク（チェックリスト）
- [ ] `domain/link` 導入と `ArticleLink` 定義
- [ ] 保存・検索APIの汎用化（`article_links` ベース）
- [ ] RSS抽出関数の返り値を `ArticleLink` に統一
- [ ] app/workflow・articleクエリの参照更新
- [ ] マイグレーション 003 を追加
- [ ] Fixtures の INSERT 先を `article_links` に変更
- [ ] すべてのテストを更新し `cargo test` をパス
- [ ] README / コメント更新

## 受け入れ条件
- `cargo test` がグリーン
- 既存データが失われず新スキーマに移行済み
- 未処理リンク取得〜記事取得フローが動作
- README に新名称・`source` の説明が反映

## リスク / メモ
- 影響範囲が広いため、変更漏れに注意（関数名・テーブル名・fixtures）
- `sqlx` の fixtures パスは相対指定のため、ファイルの置き場所変更時はパスを要確認
- `articles` とのJOINはURLベース（FKは現状なし）。必要なら今後FK設計を検討