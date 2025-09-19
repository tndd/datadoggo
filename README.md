# Datadoggo
webからニュース等のを集め、保存・分析を行う。

# アーキテクチャ
```
src
├── core      # ドメインロジック（記事、リンク、RSS処理）
├── infra     # インフラストラクチャ層（DB、API、ファイル操作）
└── workflow  # ビジネスワークフロー
```

## 注意
- `tests`ディレクトリは存在しない。`#[cfg(test)]`方式を採用している。

# 環境変数の設定
このプロジェクトは**環境変数でのテスト・本番切り替え**の仕組みがあります。
初見での混乱を避けるため、以下を必ず理解してください。

```bash
# .envファイルに以下が設定されている必要があります
DATABASE_URL=postgresql://datadoggo:datadoggo@localhost:16432/datadoggo_test
DATABASE_URL_PROD=postgresql://datadoggo:datadoggo@localhost:15432/datadoggo
```

## 環境変数`ENVIRONMENT`による切り替えの仕組み
- `ENVIRONMENT=prod` → `DATABASE_URL_PROD`を使用（本番/開発用DB）
- 指定なき場合はテストモードとして動作する

# Justfile
Justfileを使用した開発ワークフローを採用している。
基本的なコマンドは以下の通り。

## setup
コンテナの立ち上げからマイグレーションまでを行う。
```bash
just setup         # デフォルト：テスト用DBのみ
just setup prod    # 本番DBのみ
just setup all     # すべてのDBコンテナを起動・マイグレーション
```

### clearフラグによるDBの削除
- clearフラグは既存のDBの内容を削除しながら再セットアップを行う。
- prod環境のDBの削除が含まれる場合、確認プロンプトが表示される。
```bash
# 暗黙的
just setup --clear       # テスト用DBの内容を削除し再構成（暗黙的）
# 明示的
just setup test --clear  # テスト用DBの内容を削除し再構成（明示的）
just setup prod --clear  # 本番用DBの内容を削除し再構成（注意！）
```

## lint
fmt + check + clippyという3つのチェックを行う。
```bash
just lint
```

## test
compose up + lintを行い、テストを実行する・
```bash
just test
```

# VSCode Test Explorerへの対応
VSCodeのTest Explorerを有効にするには、`.vscode/settings.json`に以下の設定が必要。
```bash
DATABASE_URL="postgresql://datadoggo:datadoggo@localhost:16432/datadoggo_test"
```