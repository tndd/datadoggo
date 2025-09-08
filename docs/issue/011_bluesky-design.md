# 011: Bluesky RSS → article_links 実装計画 (2025-09-08)

**目的**
- Blueskyの「RSS表示」からポストを取得し、各ポスト本文の`<description>`内に含まれる「記事URL」を抽出してDBに保存する。
- 保存先は`migrations/001_create_article_links.sql`の`article_links`テーブル。`source`は`"bluesky"`をセットする。
- フィードの論理グループは`bluesky`とする（`config/feeds.yaml`に追加予定）。

**対象スキーマ（格納先）**
- テーブル: `article_links`
  - `url TEXT PRIMARY KEY`
  - `title TEXT NOT NULL`
  - `pub_date TIMESTAMPTZ NOT NULL`
  - `source TEXT NOT NULL`
- 本機能では以下で投入:
  - `url`: `<description>`から抽出した「記事」URL（短縮URL可）
  - `title`: `<description>`からURLを除去したテキストを整形・短縮したもの（後述の規則）
  - `pub_date`: `<item><pubDate>`を`Utc`に正規化
  - `source`: 文字列リテラル`"bluesky"`

---

## 入力データの想定
- 形式: RSS 2.0
- 例: `mock/bluesky/bloomberg.rss` / `mock/bluesky/reuter.rss`
  - `<item><link>`はポスト（`https://bsky.app/profile/.../post/...`）へのリンク
  - 実際に欲しいのは`<item><description>`中に含まれる記事URL（短縮URLが多い: `bloom.bg`, `reut.rs` など）
  - `pubDate`例: `08 Sep 2025 03:13 +0000`（曜日無しでも`dateparser`で解釈可能）
  - 一部ポストは記事URLを含まない（例: `reuter.rss`内のハッシュタグ投稿）。この場合はスキップ対象。

---

## 仕様: 抽出・正規化
- URL抽出（`<description>`）
  - 手順: テキストから`https?://[^\s<>"']+`を正規表現で抽出
  - フィルタ: `bsky.app`ドメインは除外（ポスト自身のURLは不要）
  - 選択: 複数URLがある場合は「最後のURL」を記事URLとみなす（サンプルでは末尾が記事URLの傾向）
  - 重複: 同一URLが複数回出現しても1件として扱う
  - 未検出: URLが1つも取れない`<item>`はスキップ

- タイトル生成
  - もとの`<item><title>`が無いケースが多い（サンプル）。そのため`<description>`からURLを除去し、残りテキストを**前方から最大120文字**にトリムして`title`とする
  - 余分な改行/連続空白は1スペースに圧縮
  - 完全空文字になった場合は`"(no title)"`（英語or日本語は後続要件に応じて検討。初期は`"(no title)"`で実装）

- 日付
  - `infra::parser::parse_date`で`Utc`へ正規化
  - 想定形式: `DD Mon YYYY HH:MM +0000` を含む多様なRSSスタイル

- ソース
  - `source = "bluesky"`

---

## モジュール設計（プロジェクトの一方向ルール順守）

graph LR
    infra --> core
    core --> task
    task --> app

### infra
- `src/infra/extract.rs`（新規）
  - `get_urls_from_text(text: &str) -> Vec<String>`: 正規表現でURL抽出
  - `get_last_http_url(urls: &[String]) -> Option<String>`: 最後のHTTP(S)を返す
  - `strip_urls(text: &str) -> String`: テキストからURLを除去し空白を整形
  - 備考: 既存の`infra/parser.rs`はXML/日付専用を維持し、テキスト抽出は分離

### core
- `src/core/bluesky.rs`（新規）
  - `use crate::core::rss::ArticleLink;`（既存構造体を流用）
  - `pub fn get_article_links_from_bluesky_channel(channel: &rss::Channel) -> Vec<ArticleLink>`
    - 各`<item>`について`<description>`から記事URLを抽出し、上記仕様で`ArticleLink`を生成
  - `pub async fn get_article_links_from_bluesky_feed<H: HttpClient>(client: &H, feed: &Feed) -> Result<Vec<ArticleLink>>`
    - `client.fetch(&feed.rss_link, ...)` → `parse_channel_from_xml_str` → `get_article_links_from_bluesky_channel`

### task
- `src/task/bluesky.rs`（新規）
  - `pub async fn task_collect_bluesky_article_links<H: HttpClient>(client: &H, feeds: &[Feed], pool: &PgPool) -> Result<()>`
  - `core::bluesky::get_article_links_from_bluesky_feed`で抽出 → `core::rss::store_article_links`で保存

### app
- `src/app/workflow_bluesky.rs`（新規）
  - `pub async fn execute_bluesky_workflow<H: HttpClient, F: FirecrawlClient>(http: &H, firecrawl: &F, pool: &PgPool) -> Result<()>`
  - `search_feeds(Some(FeedQuery::from_group("bluesky")))`で取得 → 上記task実行 → 既存`task_collect_articles`で本文取得
  - 既存`execute_rss_workflow`は変更しない（後方互換のため分離）

### config
- `config/feeds.yaml`に`bluesky`グループを追加
  - 例（ダミー）:
    - `bluesky.bloomberg: <実運用のRSS化サービスURL>`
    - `bluesky.reuters: <実運用のRSS化サービスURL>`
  - テストではHTTP通信を使わず、`mock/bluesky/*.rss`を`load_channel_from_xml_file`で直接読み込むユニットテストを主とする

---

## テスト方針（cargo testで完結）

優先度の高い検証観点のみ厳選（各関数/構造体あたり最大5件）。

### helper
- `infra/extract.rs`（tests/helper）
  - URL抽出: 単一/複数/重複/無し
  - 末尾選択: 先頭が記事URLでない場合でも最後を選べること
  - URL除去: テキスト整形（改行・空白圧縮）

### get_article_links_from_bluesky_channel
- `mock/bluesky/bloomberg.rss`
  - すべての抽出結果が`bsky.app`以外のURLであること
  - `source == "bluesky"`
  - `pubDate`がUTCでパースできていること
- `mock/bluesky/reuter.rss`
  - ハッシュタグのみ等、URLが無い`<item>`はスキップされること
  - 重複URL行でも1件として扱うこと

### store_article_links
- `source = "bluesky"`でUPSERTできること（既存実装の再利用。重複時はタイトル/日付/ソースが更新）

### online（任意・通常無効, `--features online`）
- 実際のRSS化エンドポイントに対する疎通試験（安定供給元が決まった後に追加）

---

## 受け入れ基準（Acceptance Criteria）
- `mock/bluesky/bloomberg.rss`/`reuter.rss`を使ったユニットテストが通る
- `get_article_links_from_bluesky_channel`の結果が以下を満たす
  - `url`は`bsky.app`以外のHTTP(S)で、`description`末尾URLを優先している
  - `title`はURL削除後に最大120文字へ整形（空なら`"(no title)"`）
  - `pub_date`はUTCに正規化されている
  - `source`は`"bluesky"`
- `store_article_links`でDB保存でき、重複時に更新判定が正しく働く
- `cargo test`が警告無しで成功

---

## 実装タスク
1) `infra/extract.rs`新規: URL抽出/整形ヘルパーの実装 + 単体テスト
2) `core/bluesky.rs`新規: Channel→ArticleLink変換 + フィード取得関数 + 単体テスト
3) `task/bluesky.rs`新規: フィード反復→DB保存（`store_article_links`再利用）
4) `app/workflow_bluesky.rs`新規: `group=bluesky`でfeed選択→リンク収集→記事取得
5) `config/feeds.yaml`に`bluesky`グループを追加（実運用URLは別Issueで確定）
6) README更新: Blueskyセクションに使い方を簡潔に追記

---

## リスク/未決事項
- RSS化エンドポイント（実運用）の選定
  - 代替案: 初期は短縮URLをそのまま保存し、本文取得時にリダイレクト追従で最終URLへ（既存の本文取得ロジックで自然に吸収できる）
- `description`の体裁ゆらぎ
  - 末尾URL優先のヒューリスティクスで吸収。将来はドメインのホワイトリストやOGP解決などを検討

---

## 運用フロー（最小）
- オフライン: `cargo test`で`mock/bluesky/*.rss`に対する抽出を検証
- オンライン（任意）: `--features online`で疎通確認を追加
- 本番: `execute_bluesky_workflow`により`group=bluesky`を処理→`article_links`→`articles`

---

## メモ
- 今日の日付は2025-09-08（絶対日付で明記）。
- Rustは最終的に`cargo test`での確認を必須（`cargo check`のみは禁止）。
- ネーミング規約: API取得は`fetch_*`、ファイルは`load_*`、DB検索は`search_*`、計算/変換は`get_*`に準拠。

以上の方針で実装を進めます。仕様の微修正（タイトル整形の最大長、オンライン疎通先の確定など）があれば指示ください。
