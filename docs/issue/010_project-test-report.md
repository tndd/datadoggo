# プロジェクト横断テスト評価レポート（2025-09-08）

本レポートは、リポジトリ内のテスト実装状況を横断的に点検し、AGENTSポリシー（本タスク定義のテスト方針）との乖離点を列挙・評価したものです。併せて、再構成後のテスト編成案を提示します。

---

## 評価基準（抜粋）
- testsモジュール配下での一元管理と区分
  - tests直下に helper / online / {関数名} / それ以外（tests）で分類
  - 外部通信が走るものは feature flag "online" で通常実行から除外
- テスト件数上限
  - 1つの関数/構造体/トレイトにつき最大5件
- 価値の高いテストを優先（境界・内部仕様に依存する壊れやすい箇所）
- フロントエンドは playwright mcp（本プロジェクトでは該当なし）

---

## 現状サマリ
- インラインテスト（`#[cfg(test)] mod tests`）を持つファイル数: 15
  - `src/app/workflow.rs`
  - `src/task/rss.rs`
  - `src/task/article.rs`
  - `src/core/rss.rs`
  - `src/core/feed.rs`
  - `src/core/article/{prelude,repository,service,builders,types}.rs`
  - `src/infra/{api/http,api/firecrawl,compute,parser,storage/file}.rs`
- `sqlx` フィクスチャ配置が分散
  - `src/core/article/fixtures/`, `src/core/fixtures/`, `src/task/fixtures/` に散在
- オンラインテスト
  - `feature = "online"` で適切に分離されている箇所が多い（良い点）
- 件数上限
  - 多くの関数で5件以内に収まっている（良い点）

---

## 乖離点（ファイル別）

以下では「どのファイルの、どの対象が、どのように基準に違反/未準拠か」を列挙します。

- `src/app/workflow.rs`
  - 対象: `execute_rss_workflow`
  - 乖離: tests配下ではなくインライン。testsモジュール階層（{関数名}サブモジュール）不使用。
  - 備考: 実ファイル `config/feeds.yaml` に依存しており、外部I/Oの影響を受けやすい。fixtures化推奨。

- `src/task/rss.rs`
  - 対象: `task_collect_article_links`
  - 乖離: インラインテスト。複数テストがあるが `{関数名}` モジュール名によるグルーピング不徹底（一部 `concurrent_processing_tests` など任意名）。

- `src/task/article.rs`
  - 対象: `task_collect_articles`
  - 乖離: インラインテスト。`error_recovery_tests` など任意名でのネスト。tests配下への集約未実施。
  - 件数: 5件（上限内）

- `src/core/rss.rs`
  - 対象: `get_article_links_from_channel`, `get_article_links_from_feed`, `store_article_links`, `search_article_links`
  - 乖離: インラインテスト。サブモジュール名は機能別（xml_parsing_tests / save_tests / retrieval_tests）であり `{関数名}` 厳密準拠でない。
  - 件数: `search_article_links` は5件（上限ギリギリ）
  - 補足: モックXML/HTTPの使い分けは良好。フィクスチャは `src/core/fixtures/` などに分散。

- `src/core/article/repository.rs`
  - 対象: `search_article_url_statuses`, `search_article_join_rows`, `search_article_contents`, `store_article_content`
  - 乖離: インラインテスト。testsモジュール外。
  - 備考: テストは機能名モジュールで良く分割されているが、配置が基準外。

- `src/core/article/service.rs`
  - 対象: `get_article_content`, `get_article_content_with_client`, `fetch_and_store_article{,_with_client}`
  - 乖離: インラインテスト。`mod helper` を内包しており「tests直下にヘルパー関数を定義しhelperモジュールでテスト」の方針に未準拠。
  - 良点: `#[cfg(feature = "online")]` でオンライン分離済み。

- `src/core/article/prelude.rs`
  - 対象: `search_articles`
  - 乖離: インラインテスト。tests配下未集約。

- `src/infra/api/http.rs`
  - 対象: `HttpClient` 実装（`ReqwestHttpClient`, `MockHttpClient`）
  - 乖離: インラインテスト。tests配下未集約。
  - 良点: オンライン小テストは `feature = "online"` で分離済み。

- `src/infra/api/firecrawl.rs`
  - 対象: `FirecrawlClient` 実装（`ReqwestFirecrawlClient`, `MockFirecrawlClient`）
  - 乖離: インラインテスト。tests配下未集約。
  - 良点: オンライン小テストは `feature = "online"` で分離済み。

- `src/infra/{compute,parser,storage/file}.rs`
  - 対象: ユーティリティ群
  - 乖離: インラインテスト。ヘルパー系は tests/helper に分離すべき。

- フィクスチャ配置の分散（横断）
  - 対象ディレクトリ: `src/core/article/fixtures/`, `src/core/fixtures/`, `src/task/fixtures/`
  - 乖離: `sqlx::test(fixtures(...))` の相対パス見通しが悪く、tests配下の一元管理基準に未準拠。

---

## リスク/問題点（運用視点）
- インライン配置により
  - モジュール分割・リネーム時に `#[sqlx::test(fixtures(...))]` の相対参照が壊れやすい
  - テストの横断検索・重複排除・共通化が難しい
- 実ファイル依存（例: `feeds.yaml`）により、環境差分の影響を受けやすい
- フィクスチャ散在により、データ意図とテスト対象の対応が追いにくい

---

## 再構成方針（編成案）

- tests 配下の構成
  - `tests/mod.rs`
    - `mod helper;`（ユーティリティ検証: `compute`, `parser`, `storage/file` のヘルパーをここでテスト）
    - `mod online;`（`cfg(feature = "online")` で全体をガード）
    - `mod execute_rss_workflow;`
    - `mod task_collect_article_links;`
    - `mod task_collect_articles;`
    - `mod get_article_links_from_channel;`
    - `mod get_article_links_from_feed;`
    - `mod store_article_links;`
    - `mod search_article_links;`
    - `mod repository_search_article_url_statuses;`
    - `mod repository_search_article_join_rows;`
    - `mod repository_search_article_contents;`
    - `mod repository_store_article_content;`
    - `mod service_get_article_content;`
    - `mod service_fetch_and_store_article;`

- ファイル例（抜粋）
  - `tests/helper.rs`
    - 対象: `infra/compute.rs`, `infra/parser.rs`, `infra/storage/file.rs` の純粋関数群
  - `tests/online.rs`
    - 対象: `infra/api/http`, `infra/api/firecrawl`, `core/article/service` のオンライン系
    - ガード: `#![cfg(feature = "online")]`
  - `tests/task_collect_article_links.rs`（関数名単位の集約）
  - `tests/task_collect_articles.rs`
  - `tests/execute_rss_workflow.rs`（アプリ層。feedsはfixtures化）
  - `tests/search_article_links.rs`（既存5件→境界/代表のみ3件程度に集約）

- フィクスチャの一元化
  - `tests/fixtures/sqlx/` に統合
  - 例: 現在の以下を移設
    - `src/core/fixtures/rss*.sql` -> `tests/fixtures/sqlx/rss*.sql`
    - `src/core/article/fixtures/*.sql` -> `tests/fixtures/sqlx/article_*.sql`
    - `src/task/fixtures/*.sql` -> `tests/fixtures/sqlx/task_*.sql`
  - `#[sqlx::test(fixtures("…"))]` は `tests/fixtures/sqlx` をデフォルト探索にする前提で命名を調整
    - 例: `fixtures("rss_edge_cases")`, `fixtures("article_mixed")` 等

- 実ファイル依存の排除
  - `execute_rss_workflow` の feeds 入力は yaml を fixtures 化し、`infra/storage/file::load_yaml_from_file` をモック差し替え or 専用ローダに分離

- テスト件数の最適化（価値優先）
  - `search_article_links` は代表・境界・エッジ（大文字小文字/うるう年）を残し5→3～4に圧縮
  - 似通ったカバレッジの重複主張を削減

---

## 具体的な移行マッピング（例）

- `src/task/rss.rs`（インライン）
  - -> `tests/task_collect_article_links.rs`
  - モジュール内の `success` / `errors` / `duplicate_handling` / `concurrent_processing` を同ファイルに集約

- `src/task/article.rs`（インライン）
  - -> `tests/task_collect_articles.rs`
  - `error_recovery_tests` を `{関数名}` モジュール配下にフラット化

- `src/app/workflow.rs`（インライン）
  - -> `tests/execute_rss_workflow.rs`
  - `feeds.yaml` 依存を fixtures 化。HTTP/Firecrawl は既存モックを利用

- `src/core/rss.rs`（インライン）
  - -> `tests/get_article_links_from_channel.rs`, `tests/get_article_links_from_feed.rs`, `tests/store_article_links.rs`, `tests/search_article_links.rs`
  - `xml_parsing_tests` / `save_tests` / `retrieval_tests` を関数単位に再編

- `src/core/article/repository.rs`（インライン）
  - -> `tests/repository_*.rs`（関数ごと）

- `src/core/article/service.rs` / `src/infra/api/{http,firecrawl}.rs`
  - -> `tests/service_get_article_content.rs`, `tests/http_client.rs`, `tests/firecrawl_client.rs`
  - オンライン系は `tests/online.rs` に移設し `cfg(feature = "online")`

- `src/infra/{compute,parser,storage/file}.rs`
  - -> `tests/helper.rs`

---

## 実施チェックリスト
- テスト配置
  - [ ] すべてのインライン `mod tests` を削除し、tests配下に再配置
  - [ ] `{関数名}` 単位でファイル化（複数テストがある関数のみ）
  - [ ] 単一テストの関数は `tests` 直下へ
- フィクスチャ
  - [ ] `tests/fixtures/sqlx` に統合し、`fixtures("…")` 名称を揃える
  - [ ] 使われないSQLは削除・統合
- オンライン
  - [ ] `tests/online.rs` を `#![cfg(feature = "online")]` で一括制御
  - [ ] 通常 `cargo test` では実行されないことを確認
- 検証
  - [ ] ローカルで `cargo test` 実行（DB/環境変数設定を明示）
  - [ ] 警告解消（unused import, dead code など）

---

## 付記（実行・環境メモ）
- DB を用いる `sqlx::test` は Postgres 接続と `./migrations` 実行が前提
  - `.env` の `DATABASE_URL` を設定
  - 例: `postgres://postgres:postgres@localhost:5432/datadoggo_test`
- 標準の実行
  - 通常: `cargo test`
  - オンライン含む: `cargo test --features online`

---

## 結論
- 現状のテストは「量・観点」は充実している一方で、AGENTSの分類基準（tests配下への一元管理、モジュール命名、fixtures一元化）に未準拠です。
- 上記の再構成を行うことで、
  - 機能単位で見通しが良くなる
  - fixtures 参照の相対パス問題を解消
  - オンライン/ヘルパーの責務が明確化
- 後方互換性は不要との前提のため、一括でtests配下に整理・移設することを推奨します。

