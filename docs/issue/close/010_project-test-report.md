# プロジェクト横断テスト評価レポート（2025-09-08）

本レポートは、リポジトリ内のテスト実装状況を横断的に点検し、AGENTSポリシー（本タスク定義のテスト方針）との乖離点を列挙・評価したものです。併せて、再構成後のテスト編成案を提示します。

---

## 評価基準（本プロジェクトの最終方針を反映）
- テスト配置は「各ファイル内の `#[cfg(test)] mod tests` マクロ方式」を維持する
- `#[sqlx::test(fixtures(...))]` は「同階層の fixtures からのみ」参照する（簡素な相対指定）
- 外部通信が走るものは feature flag "online" で通常実行から除外
- テスト件数上限: 1つの関数/構造体/トレイトにつき最大5件
- 価値の高いテストを優先（境界・内部仕様に依存する壊れやすい箇所）
- rss/link.yml の実ファイル依存は現状維持（今は変更しない）

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

## 乖離点（ファイル別・プロジェクト方針に即した観点）

以下では「どのファイルの、どの対象が、どのように基準に違反/未準拠か」を、
“インラインテスト維持・fixturesは同階層参照” を前提に列挙します。

- `src/task/rss.rs`
  - 対象: `collect_article_links_with_rss_links`
  - 乖離: テストサブモジュール名が `{関数名}` ベースで統一されていない（例: `common_concurrent_processing_tests`）。
  - 提案: `mod collect_article_links_with_rss_links { ... }` 配下に「成功/エラー/重複/並行」の4テストを整理し、名前規約を統一。

- `src/task/article.rs`
  - 対象: `collect_backlog_articles_with_firecrawl`
  - 乖離: `error_recovery_tests` のような任意名の中間モジュールがあり、関数名ベースの粒度とずれている。
  - 提案: `mod collect_backlog_articles_with_firecrawl { basic, mixed, error_reprocessing, partial_failure, mixed_result }` のようにフラット化。

- `src/core/rss.rs`
  - 対象: `get_article_links_from_channel`, `get_article_links_from_feed`, `store_article_links`, `search_article_links`
  - 乖離1: サブモジュール名が機能カテゴリ（`xml_parsing_tests` / `save_tests` / `retrieval_tests`）であり、{関数名} 基準と乖離。
  - 乖離2: `search_article_links` のテスト数が5件を超過（境界/精度/大文字小文字/エッジなど多数）。
  - 提案: {関数名} ごとに `mod get_article_links_from_channel { … }` 等へ再編し、`search_article_links` は価値の高い5件に厳選（例: 基本, パターン, 日付境界, 大文字小文字, 代表エッジ）。

- `src/core/article/repository.rs`
  - 対象: `search_article_url_statuses`, `search_article_join_rows`, `search_article_contents`, `store_article_content`
  - 乖離: 概ね良好だが、関数名ベースのモジュール名に明示揃えがあると可読性が上がる（現在は意図は揃っているが命名タグにばらつき）。
  - 提案: `mod search_article_url_statuses { … }` 等の明示的モジュール名に統一。

- `src/core/article/service.rs`
  - 対象: `fetch_article_content`（DI）, `fetch_and_store_article`（DI）
  - 乖離: `mod helper` が `tests` 内に内在し、他テストと混在。helperテストは `tests` 配下で `mod helper` を切る代わりに、同ファイル内でも `mod helper` を最初に置き、以降 `{関数名}` モジュールと明確に分離（命名・並び順の統一）。
  - 良点: `#[cfg(feature = "online")]` の分離は適切。

- `src/core/article/prelude.rs`
  - 対象: `search_articles`
  - 乖離: テストは妥当だが、モジュール名を `{関数名}` に合わせておくと横断検索性が増す。

- `src/infra/api/http.rs`, `src/infra/api/firecrawl.rs`
  - 対象: クライアント実装
  - 乖離: オンライン・オフラインのテストが同一 `mod tests` 配下に混在。
  - 提案: `mod online` を tests 内部で明確化し、`#[cfg(feature = "online")]` をモジュール入口に付与して「目視での切替範囲」を分かりやすくする。

- `src/infra/{compute,parser,storage/file}.rs`
  - 対象: ヘルパー/ユーティリティ
  - 現状: テスト数・内容ともに適正。`mod helper` を tests の先頭に置く運用ルールを全ファイルで徹底（読み手がまずヘルパーを把握できる）。

- フィクスチャ
  - 現状: `src/core/article/fixtures/`, `src/core/fixtures/`, `src/task/fixtures/` と“近接配置”で運用。
  - 方針: 一元化は行わない。各テストは「同階層 fixtures のみ」を参照するように維持（既存設計に適合）。

---

## リスク/問題点（運用視点）
- インライン配置により
  - モジュール分割・リネーム時に `#[sqlx::test(fixtures(...))]` の相対参照が壊れやすい
  - テストの横断検索・重複排除・共通化が難しい
- 実ファイル依存（例: `rss/link.yml`）により、環境差分の影響を受けやすい
- フィクスチャ散在により、データ意図とテスト対象の対応が追いにくい

---

## 再構成方針（“インライン方式を維持”したままの編成案）

- 各ソースファイル内 `mod tests` の標準レイアウトを統一
  - 先頭: `mod helper`（ある場合）
  - 次: `mod online`（`#[cfg(feature = "online")]` をモジュールに付与）
  - 続けて: `{関数名}` ごとのサブモジュール（複数テストがある関数のみモジュール化）
    - 例: `mod collect_article_links_with_rss_links { success, errors, duplicate, concurrent }`
    - 例: `mod search_article_links { basic, pattern, date_boundary, case_insensitive, edge }`（最大5件）

- フィクスチャ運用
  - 既存通り“同階層”に配置し、`#[sqlx::test(fixtures("…"))]` の相対参照は簡素なまま維持
  - フィクスチャ名の命名規約のみ整理（例: `rss_*.sql`, `service_*.sql`, `task_*.sql`）

- rss/link.yml 依存
  - 現状維持（`app/workflow.rs` の統合テストは実ファイルを使用）

- テスト件数の最適化（価値優先）
  - `search_article_links` など5件超の箇所は代表性の高い5件に厳選
  - 近い観点の重複は統合（例: 日付精度と境界の一部をまとめる）

---

## 具体的な修正マッピング（同一ファイル内での再編）

- `src/task/rss.rs`
  - `mod tests` 内を `mod collect_article_links_with_rss_links { … }` に再編（サブテスト: success/errors/duplicate/concurrent）

- `src/task/article.rs`
  - `error_recovery_tests` を `mod collect_backlog_articles_with_firecrawl { … }` 配下に統一（basic/mixed/error_reprocessing/partial_failure/mixed_result）

- `src/app/workflow.rs`
  - 変更なし（rss/link.yml 依存は現状維持）。`mod online` の位置づけ明確化のみ検討可

- `src/core/rss.rs`
  - `xml_parsing_tests` → `mod get_article_links_from_channel { … }`
  - `save_tests` → `mod store_article_links { … }`
  - `retrieval_tests` / `edge_cases` → `mod search_article_links { … }` に統合し、テストは最大5件へ厳選

- `src/core/article/repository.rs`
  - `mod search_article_url_statuses`, `mod search_article_join_rows`, `mod search_article_contents`, `mod store_article_content` に命名を明示統一

- `src/core/article/service.rs`
  - `mod helper` を tests 先頭に配置し、その後に `{関数名}` を並べる

- `src/infra/api/{http,firecrawl}.rs`
  - `mod online` を tests 内で明示化し、モジュール全体に `#[cfg(feature = "online")]` を付ける

---

## 実施チェックリスト
- テストレイアウト
  - [ ] 各ファイルの `mod tests` で、`helper` → `online` → `{関数名}` の順に統一
  - [ ] `{関数名}` サブモジュール名を関数実体に揃える
  - [ ] 各対象のテスト件数は最大5件に収める（特に `search_article_links`）
- フィクスチャ
  - [ ] すべて「同階層」参照で成立しているか確認（`fixtures("…")`）
  - [ ] フィクスチャ命名を用途別（rss_*, service_*, task_*）に整える
- オンライン
  - [ ] `mod online` をモジュール単位で `#[cfg(feature = "online")]` にする
  - [ ] 通常の `cargo test` で実行されないことを確認
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
- 現状のテストは「量・観点」は充実しており、プロジェクト方針（インライン方式・同階層fixtures）にも概ね適合しています。
- 主要な改善点は「命名とサブモジュール構成の統一」と「1対象あたり最大5件への厳選」です。特に `src/core/rss.rs` の `search_article_links` は代表ケースへの絞り込みが必要です。
- rss/link.yml 依存は現状維持としつつも、将来的に必要であればフィクスチャ化の選択肢は残せます（今回の範囲では不変更）。

---

## 付録: 現時点の追加テスト必要性とfixtures再利用の評価

- 追加テストの必要性
  - 層（infra/core/task/app）ごとに基本・境界・エラー系が既にカバーされており、現時点でテスト数の大幅な増強は不要。
  - 例外的に、未直テストの関数（`fetch_and_store_article`）は他経路（task/app統合）で間接カバーされているため必須ではないが、将来の変更点として1件だけのスモーク追加は検討余地あり（必須ではない）。

- 無駄なテスト/重複
  - 顕著な重複は `src/core/rss.rs` の `search_article_links` 周辺に集中（類似観点が分割され件数過多）。提案した5件への統合で解消可能。
  - それ以外のモジュールでは明確な重複・冗長性は目立たず、意味の重なりも許容範囲。

- fixtures SQL の使い回し
  - フィクスチャは“モジュール近接・同階層参照”の方針に沿っており、過度な使い回しは見当たらない。
  - `task` 配下で `common_concurrent_processing`/`common_error_recovery_scenarios` を複数テストから参照しているが、対象ドメインが同一であり妥当。
  - 参照されていない孤立フィクスチャは現状なし（grep調査済み）。
