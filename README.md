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

## SQLXキャッシュの重要性
- `SQLX_OFFLINE=true`でオフラインモード（キャッシュファイル必須）
- キャッシュ更新: `just sqlx_prepare`（テスト用DBが必要）
- キャッシュファイル: `.sqlx/query-*.json`（コミット対象）

### 4. Dockerコンテナのポート分離
- **本番/開発用DB**: ポート15432
- **テスト用DB**: ポート16432
- 両方のコンテナが必要な理由：並行実行でのデータ競合防止

## 開発ワークフロー
```bash
# 1. 初回セットアップ
just setup both    # 両方のDBコンテナを起動・マイグレーション

# 2. 開発時の品質チェック
just lint          # fmt + check + clippy

# 3. テスト実行
just test          # lint + テスト実行

# 4. SQLクエリ変更時
just sqlx_prepare  # キャッシュ更新（必須）
```

### よくあるエラーと解決方法
1. **`SQLX_OFFLINE` エラー** → `just sqlx_prepare`でキャッシュ更新
2. **DB接続エラー** → `just setup both`でコンテナ確認
3. **pre-commitエラー** → fmtによる自動修正後、`git add .`して再コミット
