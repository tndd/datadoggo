# コミット時に `cargo fmt` を自動実行する（ローカル pre-commit のみ）

種別: ツーリング / DX 改善
優先度: 中

## 要約
- コミット前フックで `cargo fmt --all` を自動実行し、未整形差分があればコミットをブロックする。

## 方針（確定）
- `pre-commit` フレームワークを採用し、設定ファイル（`.pre-commit-config.yaml`）のみを追加する。
- 各開発者が `pre-commit install` を実行すれば有効化される（README への追記は行わない）。

## 実装
1) リポジトリ直下に `.pre-commit-config.yaml` を追加

```
repos:
  - repo: local
    hooks:
      - id: rustfmt
        name: rustfmt
        entry: cargo fmt --all
        language: system
        types_or: [rust, toml]
        pass_filenames: false
```

補足
- エディタの自動整形は任意。最終的な整形保証は pre-commit が行う。

## 作業チェックリスト
- [ ] `.pre-commit-config.yaml` を追加（`cargo fmt --all` のみ）
- [ ] コミット前フックの動作確認（未整形でコミットが失敗すること／整形後は成功すること）

## 受け入れ条件
- 未整形ファイルを含むコミットが失敗する。
- 整形済みのコミットは成功する。

## リスク / メモ
- `pre-commit` を未導入の環境ではフックが動作しないため、開発環境セットアップの一環として導入を必須化する。
- `rustfmt` バージョン差による微差分は将来的に `rust-toolchain.toml` で固定化して回避可能。