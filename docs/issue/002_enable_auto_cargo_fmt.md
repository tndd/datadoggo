# コミット時に `cargo fmt` を自動実行する

- 種別: ツーリング / DX 改善
- 優先度: 中
- 影響範囲: リポジトリルート、開発者ワークフロー、CI

## 背景 / 課題

コミット時に `cargo fmt` を自動で実行することで、コードスタイルのばらつきを防ぎ、レビューコストを下げたい。現状、このリポジトリには pre-commit hook や `pre-commit` フレームワークの導入は見当たらない（検索結果に該当なし）。

## 目標

- ローカルでのコミット前に `cargo fmt` が自動実行され、未整形差分がある場合はコミットをブロックする
- CI でも `cargo fmt --all -- --check` を実行して、hook をバイパスしたコミットも検知する

## アプローチ候補

A) Git hooks（手動設置）
- `.git/hooks/pre-commit` にシェルスクリプトを配置
- 長所: 依存が少なく軽量
- 短所: クローンごとに配布されず、開発者の手動セットアップが必要

B) `pre-commit` フレームワーク（推奨）
- Python 製の `pre-commit` を使い、`.pre-commit-config.yaml` を共有
- 長所: 再現性が高く配布が容易、他の便利フック（trailing-whitespace, end-of-file-fixer 等）も一緒に使える
- 短所: 追加依存が必要

C) CI のみ（最小）
- ローカルhookは導入せず、CI で `cargo fmt --all -- --check` を実行
- 長所: 導入が最小
- 短所: ローカルでの即時フィードバックがない

本リポジトリでは再現性と配布容易性を重視し、B + C の併用を推奨します。

## 実装計画

1) `pre-commit` 導入
- リポジトリ直下に `.pre-commit-config.yaml` を追加
- hooks: rustfmt（`cargo fmt --all`）、将来的に clippy、trailing-whitespace、end-of-file-fixer などを追加
- 導入手順（ドキュメント化）
  - `pipx install pre-commit` または `pip install pre-commit`
  - `pre-commit install`

2) GitHub Actions（既存の `.github/workflows/` に追加）
- 新規ワークフロー or 既存にジョブ追加で `cargo fmt --all -- --check` を実行
- 将来的に `cargo clippy -- -D warnings` も追加

3) フォールバック（任意）
- `pre-commit` を使わない場合のために、`.git/hooks/pre-commit` のサンプルスクリプトを `docs/` に置く

## 追加情報

- 既存ワークフロー: `.github/workflows/claude.yml`, `claude-code-review.yml` が存在（フォーマット検査は未導入）

## タスク（チェックリスト）
- [ ] `.pre-commit-config.yaml` を追加（rustfmt/メタフック）
- [ ] 貢献ガイド（README等）にセットアップ手順を追記
- [ ] CI に `cargo fmt --all -- --check` を追加
- [ ]（任意）`.git/hooks/pre-commit` サンプルを `docs/` に配置

## 受け入れ条件
- ローカルで `pre-commit install` 後、未整形ファイルを含むコミットが失敗する
- CI で `cargo fmt --check` が実行され、未整形コミットが検知される

## リスク / メモ
- 開発者が `pre-commit` をセットアップしていない場合、ローカルhookは効かないため、CI 側のチェックは必須
- フォーマッタのバージョン差異による差分ズレは、`rust-toolchain.toml` の導入で将来対処可能