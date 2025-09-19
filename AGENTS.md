# Repository Guidelines

## プロジェクト構造とモジュール構成
本リポジトリは Rust 製のニュース集約サービスで、主要コードは `src` 配下に集約されています。`src/core` は記事・リンクなどのドメインモデルとクエリ、`src/infra` は Firecrawl や SQLx などインフラ統合、`src/workflow` は定期実行ジョブや RSS 処理のオーケストレーションを担います。CLI エントリは `src/main.rs`、共有 API は `src/lib.rs` が起点です。SQL マイグレーションは `migrations/`、`sqlx::test` 用フィクスチャは各モジュール直下の `fixtures/*.sql` に保管します。`docs/` には設計ノートや issue サマリ、`mock/` には外部 API のスタブ、生成物は `target/` 以下に出力されます。Docker 設定は `docker-compose.yml` を参照してください。

## ビルド・テスト・開発コマンド
初回は `just setup` で Postgres コンテナ起動とマイグレーションを整え、環境切替が必要なら `just setup prod` や `just setup all --clear` を慎重に使用します。既存コンテナが起動済みかどうかは `docker compose ps` で確認し、不要な停止を避けてください。品質チェックは `just lint` が `cargo fmt --all`, `cargo check`, `cargo clippy --all-targets --all-features` を連鎖実行します。日常の検証は `cargo test --all`、Docker を含む包括検証は `just test` を使い、compose を伴うため実行前に他プロセスとの競合がないか確認します。スキーマ更新後は `sqlx migrate run` を実行し、`.env` で `DATABASE_URL` と `DATABASE_URL_PROD` を常に同期させます。短いフィードバックが必要な場合は対象クレートを絞った `cargo test -p <crate>` や特定モジュールの `cargo test module::name` を活用してください。

## コーディングスタイルと命名規約
Rustfmt 既定の四スペースインデントを守り、コミット前に `cargo fmt --all` を必ず流します。DB 由来の複数件取得は `search_*`、単一値は `find_*`、外部 API 呼び出しは `fetch_*`、ファイル読み込みは `load_*`、既存データ変換は `get_*` に統一します。エラーハンドリングは `anyhow::Result` を用い、失敗要因は `Context` で補強してください。テストヘルパーは `tests::helper` モジュールへ集約し、コメントは簡潔な日本語で目的と検証観点を明示します。リネーム時は関連テスト名、フィクスチャ、コメントも忘れずに更新し、可読性が低いロジックには最小限の説明コメントを添えてください。

## テストガイドライン
テストは対象関数名のサブモジュールにまとめ、一関数あたり五ケース以内に厳選します。各テストには目的と検証観点をコメントで残し、バグ再発の確率が高い入力や境界値に集中させます。DB 結合テストは `#[sqlx::test(fixtures("..."))]` を用い、フィクスチャ SQL は同階層に配置してパスを簡潔に保ちます。外部通信が必要なシナリオは `tests::online` に隔離し、feature `online` を有効化した場合のみ実行されるよう `cfg(feature = "online")` を活用してください。実装後は `cargo test` と `just lint` をローカルで完走させ、警告を残さないことが必須です。ブラウザ検証が必要な場合は Playwright MCP プロファイルを利用し、結果を PR へ添付してください。

## コミットおよびプルリク運用
コミットメッセージは `type: 要約` 形式を推奨し、`refactor: sqlx の接続設定を整理` や `docs: フィクスチャ手順を追記` を好例とします。論理的に独立した変更はコミットを分割し、リネームや整形のみのコミットも個別に切り出してください。ブランチ名は `feature/<topic>` や `fix/<issue-number>` のように目的が分かる形式を選び、`develop`ブランチから切り出してください。プルリクエストでは目的、主要変更点、実行したコマンド、既知のリスクを箇条書きし、対応 Issue があれば必ずリンクします。UI や外部挙動に影響する変更はログやスクリーンショットを添え、レビュアーが `just test` を再現できる手順と期待結果を記載してください。

## 環境変数とセキュリティ
`ENVIRONMENT` 未設定時はテスト DB を使用し、`ENVIRONMENT=prod` で `DATABASE_URL_PROD` が選択されます。資格情報は `.env` と CI シークレットに限定し、リポジトリへコミットしないでください。`docker-compose.yml` のポート割り当てはテスト用 16432、本番用 15432 を想定しているため、変更が必要な際は README とこのガイドの双方を更新します。新規依存を導入する前には既存のインストール状況を確認し、Docker イメージやシステムパッケージを変更する場合は専用 PR でレビューを受けてください。
