# Datadoggo プロジェクト初期化指針

このドキュメントは、Datadoggoプロジェクトのコーディングスタイル、アーキテクチャパターン、および開発指針を定義します。

## プロジェクト概要

DatadoggoはRSSフィードとFirecrawlデータを処理し、PostgreSQLデータベースに保存するRustアプリケーションです。

## アーキテクチャ

### ディレクトリ構造

```
src/
├── main.rs                 # エントリーポイント
├── types/                  # 共通型定義
│   ├── mod.rs
│   ├── error.rs           # 共通エラー型
│   └── result.rs          # 結果型
├── services/              # インフラストラクチャ層
│   ├── mod.rs
│   ├── db.rs             # データベース関連
│   └── loader.rs         # ファイル読み込み
├── rss.rs                 # RSSドメインロジック
└── firecrawl.rs          # Firecrawlドメインロジック
```

## エラーハンドリング原則

### 1. エラーの分離原則

**共通エラー** (`types/error.rs`):
- 複数のモジュールで使用される基盤的なエラーのみ
- ファイルI/O、データベース、JSON、設定エラーなど

**ゴ当地エラー** (各モジュール内):
- そのドメインに特化したエラーを各モジュール内で定義
- 共通エラーは`#[from]`属性を使って自動変換

### 2. エラー型の定義パターン

```rust
// 共通エラー (types/error.rs)
#[derive(Error, Debug)]
pub enum CommonError {
    #[error("ファイル操作エラー: {path} - {source}")]
    FileIo { path: String, #[source] source: std::io::Error },
    
    #[error("データベースエラー: {operation} - {source}")]
    Database { operation: String, #[source] source: sqlx::Error },
    
    // 他の共通エラー...
}

// ドメイン固有エラー (各モジュール内)
#[derive(Error, Debug)]
pub enum RssError {
    /// 共通エラーの自動変換
    #[error(transparent)]
    Common(#[from] CommonError),
    
    /// RSS固有のエラー
    #[error("RSS解析エラー: {message}")]
    Parse { 
        message: String, 
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>> 
    },
}

pub type RssResult<T> = std::result::Result<T, RssError>;
```

### 3. エラーハンドリングのベストプラクティス

- 各ドメイン関数は自分のResult型を返す (`RssResult<T>`, `FirecrawlResult<T>`)
- 共通エラーは`From`トレイトで自動変換させる
- エラー作成にはヘルパー関数を使用する
- テスト関数は`Result<(), Box<dyn std::error::Error>>`を返す

## 依存関係管理

### 必須クレート

```toml
[dependencies]
# 非同期ランタイム
tokio = { version = "1.0", features = ["macros", "rt-multi-thread"] }

# データベース
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "postgres", "macros", "migrate", "time", "chrono"] }

# シリアライゼーション
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# エラーハンドリング
thiserror = "1.0"

# 環境変数
dotenvy = "0.15"

# RSS処理
rss = "2.0"

[dev-dependencies]
ctor = "0.2"
```

## コーディング規約

### 1. モジュール構成

- **ドメインロジック**: ルートレベル (`rss.rs`, `firecrawl.rs`)
- **共通型**: `types/`ディレクトリ
- **インフラ**: `services/`ディレクトリ

### 2. エラー処理

- エラー型は必ず`thiserror::Error`を使用
- 文脈情報（ファイルパス、操作名など）を含める
- 自動変換(`#[from]`)を積極的に活用

### 3. データベース操作

- `sqlx::query!`マクロでコンパイル時型チェック
- トランザクションは必須
- 接続プールの再利用

### 4. テスト

- 統合テストは`#[sqlx::test]`を使用
- テストデータはfixtures使用
- 重複処理のテストを含める

## 初期化手順

新しいプロジェクト作成時の推奨手順：

### 1. 基本構造の作成

```bash
# プロジェクト作成
cargo new your_project --bin

# ディレクトリ作成
mkdir -p src/types src/services

# 基本ファイル作成
touch src/types/{mod.rs,error.rs,result.rs}
touch src/services/{mod.rs,db.rs,loader.rs}
```

### 2. Cargo.toml設定

上記の必須クレートを追加

### 3. 型定義の実装

- `types/error.rs`: 共通エラー型
- `types/result.rs`: 結果型
- `types/mod.rs`: 再エクスポート

### 4. インフラ層の実装

- `services/db.rs`: データベース接続とマイグレーション
- `services/loader.rs`: ファイル読み込み
- `services/mod.rs`: サービス層のエクスポート

### 5. ドメイン層の実装

- 各ドメインファイルでゴ当地エラー型を定義
- ビジネスロジックの実装
- テストの追加

## 開発ガイドライン

### DO ✅

- エラーの責任を適切に分離する
- `thiserror`を使用して型安全なエラーハンドリング
- データベースクエリには`sqlx::query!`マクロを使用
- 文脈情報を含む詳細なエラーメッセージ
- 包括的なテストカバレッジ

### DON'T ❌

- `Box<dyn std::error::Error>`の多用
- すべてのエラーを一つの巨大なenumに詰め込む
- SQLインジェクションのリスクがある動的クエリ
- トランザクションなしのデータベース操作
- エラーの文脈情報を失う処理

## マイグレーション

- データベーススキーマは`migrations/`ディレクトリで管理
- `sqlx migrate`コマンドでマイグレーション実行
- スキーマ変更は必ずマイグレーションファイルで行う

## 環境設定

```bash
# .env ファイル例
DATABASE_URL=postgresql://username:password@localhost/database_name
```

## 運用考慮事項

- エラーログには十分な文脈情報を含める
- データベース接続プールサイズの調整
- 大量データ処理時のメモリ使用量監視
- 重複データの適切な処理

---

このドキュメントは、プロジェクトの成長に合わせて更新してください。