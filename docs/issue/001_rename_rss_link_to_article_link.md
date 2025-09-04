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
  - テーブル: `rss_links` → `article_links`（複数形に確定）
  - 構造体: `RssLink` → `ArticleLink`
  - 関数名・クエリ名から `rss` 固有の語を排し抽象化（ただしRSSパース自体は`rss.rs`内に閉じ込める）
- リンクの由来を示すフィールドを追加（命名は `source` に確定）

## 提案 / 設計

1) DBスキーマ定義（初期マイグレーションを直接修正 + ファイル名をリネーム）
- 変更履歴のALTERは行わず、初期マイグレーションそのものを更新する
- ファイル名: `migrations/001_create_rss_links.sql` → `migrations/001_create_article_links.sql` に変更
- CREATE TABLE を `article_links` に変更（複数形）
- カラム定義
  - `link TEXT PRIMARY KEY`
  - `title TEXT NOT NULL`
  - `pub_date TIMESTAMPTZ NOT NULL`
  - `source TEXT NOT NULL DEFAULT 'rss'`

テーブル定義（Markdown）

| カラム名 | 型 | 制約 | 説明 |
|---|---|---|---|
| link | TEXT | PRIMARY KEY | 記事URL（ユニークキー） |
| title | TEXT | NOT NULL | 記事タイトル（RSS項目のtitle等） |
| pub_date | TIMESTAMPTZ | NOT NULL | 記事の公開日時（タイムゾーン付き） |
| source | TEXT | NOT NULL DEFAULT 'rss' | 由来（例: rss, sitemap, curation, manual） |

2) コードの変更方針（ファイル名は変更しない）
- `src/domain/rss.rs` のまま、内部のデータ型とクエリ先を更新
  - 構造体 `RssLink` → `ArticleLink` に改名（定義は同ファイルに残す）
  - 保存・検索系は `article_links` テーブルを対象にする
  - 由来カラム `source` を `ArticleLink` に追加し、保存・検索・未処理検索に反映
- 関数名は用途ベースに統一（方針B）
  - 例: `get_rss_links_from_channel` → `get_article_links_from_channel`
  - 例: `get_rss_links_from_feed` → `get_article_links_from_feed`
  - 保存・検索APIも `store_article_links` / `search_article_links` / `search_unprocessed_article_links` に改名
- `src/app/workflow.rs` と `src/domain/article.rs` のJOIN対象テーブルを `article_links` に変更

3) テスト / フィクスチャ更新
- `fixtures/*.sql` の INSERT 先を `article_links` に変更
- `#[sqlx::test(fixtures(...))]` 内のシンボルやコメントを新名称に合わせて更新
- 由来カラム `source` は既定値 `rss` があるため、フィクスチャでは省略可能（必要に応じて明示）

4) ドキュメント更新
- `README.md` のフロー説明（`rss_link` → `article_links` と 由来カラム `source` の説明）
- `migrations/002_create_articles.sql` などのコメントを現状に合わせて修正

## マイグレーション計画（開発・CI共通の原則）
- 初期マイグレーションを `001_create_article_links.sql` として定義し、テーブル `article_links` を作成
- 既存のDBに対してALTERはせず、DBを一新した上でマイグレーションを再実行
- 実行例（ローカル）: <mcfile name="reset_migrate.sh" path="/Users/tau/Repository/datadoggo/scripts/reset_migrate.sh"></mcfile> を利用可能
- CIの原則: 各ジョブは空のDBから開始し、`sqlx migrate run` 等でマイグレーションを適用してからテストを実行（ジョブ内でDB初期化を保証する）

## タスク（チェックリスト）
- [ ] `migrations/001_create_rss_links.sql` を `001_create_article_links.sql` にリネームし、テーブル定義を更新
- [ ] `src/domain/rss.rs` の `RssLink` を `ArticleLink` に改名、`source` を追加
- [ ] 保存/検索/未処理検索クエリの参照先を `article_links` に更新
- [ ] `src/app/workflow.rs`・`src/domain/article.rs` のJOINや参照を更新
- [ ] `fixtures/*.sql` のINSERT先・カラム名を更新
- [ ] README/コメント更新
- [ ] DB初期化 → マイグレーション再実行 → `cargo test` でグリーン確認（ローカル/CIとも）

## 受け入れ条件
- `cargo test` がグリーン
- 未処理リンク取得〜記事取得フローが動作
- README に新名称・由来カラムの説明が反映

## 前提（確定事項）
- テーブル名は `article_links`（複数形）
- 由来カラム名は `source`（TEXT, NOT NULL, DEFAULT 'rss'）
- 関数名は用途ベースに統一（`get_article_links_from_*` など）
- この段階ではCHECK制約/ENUM/追加インデックスは導入しない
- マイグレーションのファイル名は `001_create_article_links.sql` にリネームする

## リスク / メモ
- 影響範囲が広いため、関数名・テーブル名・fixtures の変更漏れに注意
- `sqlx` の fixtures パスは相対指定のため、ファイルの置き場所変更時はパスを要確認
- `articles` とのJOINはURLベース（FKは現状なし）。必要なら今後FK設計を検討