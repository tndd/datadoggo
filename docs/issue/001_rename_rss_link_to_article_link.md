# `rss_links` を `article_links` に統一し、由来を表す `source` を追加する

種別: リファクタリング / DB スキーマ変更（互換性破壊あり）
優先度: 高

## 要約
- RSS 前提の命名（rss_links, RssLink）を用途ベース（article_links, ArticleLink）に変更する。
- リンクの由来を保持する `source` 列を追加する（TEXT, NOT NULL, DEFAULT 'rss'）。
- ファイル名 `src/domain/rss.rs` は維持しつつ、関数名は用途ベース（article_links）に統一する。

## 仕様（確定）
### DB スキーマ（初期マイグレーションを更新）
- ファイル名を `migrations/001_create_article_links.sql` に変更し、中身を下記スキーマに更新する。
- 既存 DB への ALTER は行わず、DB を一度リセットして再マイグレーションする。

テーブル: `article_links`

| カラム名 | 型 | 制約 | 説明 |
|---|---|---|---|
| link | TEXT | PRIMARY KEY | 記事 URL（ユニーク） |
| title | TEXT | NOT NULL | 記事タイトル |
| pub_date | TIMESTAMPTZ | NOT NULL | 公開日時（TZ 付き） |
| source | TEXT | NOT NULL | 由来（例: rss, sitemap, curation, manual） |

補足: 追加の CHECK/ENUM/インデックスは現時点では導入しない。

### コード
- `src/domain/rss.rs`
  - 構造体: `RssLink` → `ArticleLink { link, title, pub_date, source }`
  - 関数名を用途ベースに変更:
    - `get_rss_links_from_channel/feed` → `get_article_links_from_channel/feed`
    - `store_rss_links` → `store_article_links`
    - `search_rss_links` → `search_article_links`
    - `search_unprocessed_rss_links` → `search_unprocessed_article_links`
  - 参照テーブルを `article_links` に変更
- `src/app/workflow.rs`, `src/domain/article.rs`
  - JOIN/参照先を `article_links` に更新（例: `ON al.link = a.url`）

### テスト / フィクスチャ / ドキュメント
- `fixtures/*.sql` の INSERT 先を `article_links` に変更（`source` は既定値 'rss' のため省略可）。
- `README.md` とコメントを新名称・`source` の説明に合わせて更新。

## マイグレーション手順（簡潔）
1) `migrations/001_create_rss_links.sql` を `migrations/001_create_article_links.sql` にリネームし、スキーマを更新。
2) DB をリセットして再マイグレーションを実行（例: <mcfile name="reset_migrate.sh" path="/Users/tau/Repository/datadoggo/scripts/reset_migrate.sh"></mcfile>）。
3) `cargo test` を実行し、全テストが通ることを確認。

## 作業チェックリスト
- [ ] `001_create_rss_links.sql` → `001_create_article_links.sql` リネーム + スキーマ更新
- [ ] `src/domain/rss.rs` の型・関数名・クエリ参照を用途ベース/`article_links` に変更（`ArticleLink` + `source`）
- [ ] `src/app/workflow.rs`, `src/domain/article.rs` の JOIN/参照を更新
- [ ] `fixtures/*.sql` の INSERT 先・カラム名を更新
- [ ] `README.md`/コメントの表記更新
- [ ] DB リセット → マイグレーション → `cargo test` グリーン確認

## 受け入れ条件
- すべてのテストがグリーン。
- 未処理リンク取得〜記事取得フローが正常に動作。
- README に `article_links` と `source` の説明が反映。