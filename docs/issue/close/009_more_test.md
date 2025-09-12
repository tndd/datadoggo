# Issue #009: sqlx fixture を用いたテストの大幅な拡充が必要

## 概要

現在のプロジェクトにおいて、`sqlx` fixture を用いたテストカバレッジが `core/article` を除き極端に不足している。特に `src/app/fixtures` が完全に空であり、アプリケーション層の品質を担保できていない状況。

## 問題の詳細

### 1. 各層の fixture 状況

| 層      | ディレクトリ                 | fixture ファイル数 | 状況         |
| ------- | ---------------------------- | ------------------ | ------------ |
| **app** | `src/app/fixtures/`          | **0 件**           | **完全に空** |
| core    | `src/core/article/fixtures/` | 7 件               | 充実（規範） |
| core    | `src/core/fixtures/`         | 2 件               | 不足         |
| task    | `src/task/fixtures/`         | 2 件               | 不足         |
| infra   | なし                         | 0 件               | fixture なし |

### 2. テスト実装状況

```bash
# grep "#[cfg(test)]" の結果
/src/core/feed.rs:94
/src/core/rss.rs:152
/src/app/workflow.rs:62
/src/task/article.rs:58
/src/infra/api/http.rs:133
/src/infra/storage/file.rs:38
/src/infra/parser.rs:45
/src/core/article/service.rs:399
/src/infra/compute.rs:43
/src/task/rss.rs:45
/src/core/article/prelude.rs:81
/src/infra/api/firecrawl.rs:100
```

**問題点:**
- `core/article` 以外は各ファイル 1 つの `#[cfg(test)]` ブロックのみ
- fixture を活用したテストが見当たらない（`include_str!` での fixture 読み込みが確認できない）

### 3. 規範となる `core/article` の fixture 構成

```
src/core/article/fixtures/
├── article_basic.sql
├── article_filter.sql
├── service_basic.sql
├── service_boundary_values.sql
├── service_comprehensive.sql
├── service_limit_tests.sql
└── service_search_patterns.sql
```

**特徴:**
- 機能別・観点別に分類された fixture
- 基本系・境界値・包括的テスト・制限値テスト・検索パターンなど多角的な観点
- 命名規則が統一されている（`{機能}_{観点}.sql`）

## 影響範囲

### 高リスク
- **app 層全般**: Workflow, UseCase の品質保証が皆無
- **core/rss, core/feed**: RSS 処理の信頼性が不十分
- **task 層**: バックグラウンド処理の品質が不安定

### 中リスク
- **infra 層**: 外部 API 連携、ストレージ操作の品質

## 対応方針

### Phase 1: app 層の緊急対応

1. **`src/app/fixtures/` の新設**
   ```
   src/app/fixtures/
   ├── workflow_basic.sql          # 基本的なワークフロー
   ├── workflow_error_cases.sql    # エラーケース
   ├── workflow_boundary.sql       # 境界値テスト
   ├── workflow_large_data.sql     # 大量データ処理
   └── article_mixed.sql          # 複合シナリオ
   ```

2. **app/workflow.rs のテスト拡充**
   - 現在の 1 テストケースを最低 5 ケースに拡張
   - fixture を活用した統合テストの実装

### Phase 2: 各層の fixture 拡充

1. **core 層**
   ```
   src/core/fixtures/
   ├── rss.sql (既存)
   ├── rss_backlog.sql (既存)
   ├── rss_error_cases.sql         # 新規
   ├── rss_large_feed.sql          # 新規
   ├── feed_basic.sql              # 新規
   ├── feed_invalid.sql            # 新規
   └── feed_boundary.sql           # 新規
   ```

2. **task 層**
   ```
   src/task/fixtures/
   ├── workflow.sql (既存)
   ├── article_mixed.sql (既存)
   ├── article_processing.sql      # 新規
   ├── rss_fetch_scenarios.sql     # 新規
   └── error_recovery.sql          # 新規
   ```

### Phase 3: テスト分類の統一

**テストモジュール構成ルールの適用:**
```rust
#[cfg(test)]
mod tests {
    mod helper {        // テスト用ヘルパー関数
        // ...
    }
    
    mod online {        // 外部通信テスト
        // ...
    }
    
    mod {関数名} {      // 関数別テスト（複数ある場合）
        // 最大5ケースまで
    }
    
    // 単発テストは直下に配置
}
```

## 完了の定義

### 必須条件
- [ ] `src/app/fixtures/` に最低 5 件の fixture を追加
- [ ] 各層で fixture ファイル数を 5 件以上に拡充
- [ ] 全モジュールでテストケース数を 3 倍以上に増加
- [ ] `cargo test` が全てグリーンで実行時間 60 秒以内

### 品質条件
- [ ] 正常系・異常系・境界値・大量データの観点を網羅
- [ ] fixture 命名規則の統一（`{機能}_{観点}.sql`）
- [ ] テストモジュール分類ルールの適用

## 優先度

**High**: app 層の fixture 新設（Phase 1）  
**Medium**: core/task 層の fixture 拡充（Phase 2）  
**Low**: infra 層のテスト拡充、CI 最適化（Phase 3）

## 備考

- fixture 増加による CI 実行時間の監視が必要
- 必要に応じて並列実行や軽量 fixture への切り替えを検討
- `core/article` の実装を参考に、他層でも同様の品質レベルを目指す