# Datadoggo
webからニュース等のを集め、保存・分析を行う。

# 環境変数の設定
このプロジェクトは**環境変数でのテスト・本番切り替え**の仕組みがあります。
初見での混乱を避けるため、以下を必ず理解してください。

```bash
# .envファイルに以下が設定されている必要があります
PROD_DB_URL=postgresql://datadoggo:datadoggo@localhost:15432/datadoggo
TEST_DB_URL=postgresql://datadoggo:datadoggo@localhost:16432/datadoggo_test
```

## 環境変数`ENVIRONMENT`による切り替えの仕組み
- `ENVIRONMENT=prod` → `PROD_DB_URL`を使用（本番/開発用DB）
- `ENVIRONMENT=test` → `TEST_DB_URL`を使用（テスト用DB）
- その他の場合はエラーとなるので注意。設定は明示的でなければならない。

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
clearフラグは既存のDBの内容を削除しながら再セットアップを行う。
```bash
just setup --clear # テスト用DBの内容を削除し再構成
just setup --all   # テスト・本番両方のDBの内容を削除し再構成

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