# SQLディレクトリ構造

このプロジェクトのSQL関連ファイルは `sql/` ディレクトリ下に統合されています。

## ディレクトリ構造

```
sql/
├── fixtures/          # テスト用フィクスチャファイル
│   ├── article/       # 記事関連のテストデータ
│   │   ├── basic.sql
│   │   ├── backlog.sql
│   │   ├── query_filter.sql
│   │   └── unprocessed.sql
│   ├── rss/           # RSS関連のテストデータ
│   │   ├── basic.sql
│   │   └── backlog.sql
│   └── workflow/      # ワークフロー関連のテストデータ
│       ├── basic.sql
│       └── mixed.sql
└── migrations/         # データベーススキーママイグレーション
    ├── 001_create_article_links.sql
    └── 002_create_articles.sql
```

## フィクスチャファイル命名規則

### 移行前の接頭辞システム（廃止）
- `article_*` → `sql/fixtures/article/`
- `rss_*` → `sql/fixtures/rss/`
- `workflow_*` → `sql/fixtures/workflow/`

### 現在のディレクトリベース構造
各カテゴリごとにディレクトリを作成し、機能別にファイルを配置：

- **article/**: 記事取得・管理機能のテストデータ
- **rss/**: RSS取得・解析機能のテストデータ  
- **workflow/**: 統合ワークフロー機能のテストデータ

## 使用方法

### テストでのフィクスチャ指定
```rust
#[sqlx::test(fixtures("../../sql/fixtures/rss/basic.sql"))]
async fn test_rss_function(pool: PgPool) -> Result<(), anyhow::Error> {
    // テスト実装
}
```

### マイグレーション実行
```bash
# スクリプト経由
./scripts/reset_migrate.sh

# 直接実行
sqlx migrate run --source sql/migrations
```

### コードでのマイグレーション指定
```rust
sqlx::migrate!("./sql/migrations")
    .run(pool)
    .await?;
```

## 利点

1. **統合された構造**: SQL関連ファイルが一箇所に集約
2. **明確な分類**: 機能別ディレクトリで管理しやすい
3. **スケーラブル**: 新しいカテゴリの追加が容易
4. **保守性向上**: 接頭辞による命名からディレクトリベースへの移行

## 変更履歴

- **Issue #7対応**: fixtures/とmigrations/をsql/下に統合
- 接頭辞ベースからディレクトリベースへの移行完了
- テストコードとスクリプトの参照パス更新完了