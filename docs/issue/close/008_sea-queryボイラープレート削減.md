# Issue #008: sea-queryクエリ構築のボイラープレート削減

## 背景・動機

Issue #007でsea-queryライブラリ移行は完了したが、動的クエリ構築部分に大量のボイラープレートが残存している。各クエリ関数で同様の条件分岐パターンが繰り返され、保守性と可読性が大幅に低下している。

### 現在の問題

```rust
// 現在のコード（典型例）
if let Some(ref url_pattern) = query.url_pattern {
    let pattern = format!("%{}%", url_pattern);
    select = select
        .and_where(Expr::col((ArticleLinks::Table, ArticleLinks::Url)).ilike(pattern))
        .to_owned();
}

if let Some(pub_date_from) = query.pub_date_from {
    select = select
        .and_where(Expr::col((ArticleLinks::Table, ArticleLinks::PubDate)).gte(pub_date_from))
        .to_owned();
}
// ... この繰り返しが約13箇所
```

**問題点**：
- 1つの関数あたり約46行のボイラープレート
- 同一パターンの重複実装（DRY原則違反）
- 条件追加時の修正コストが高い
- ビジネスロジックが冗長なコードに埋もれる

## 選定したソリューション: Helper Methods（拡張メソッド）

### 選定理由
- **最大の簡潔化効果**: 67%の行数削減（46行→15行）
- **完全なIDE補完**: 既存のsea-query APIと自然に統合
- **型安全性維持**: コンパイル時エラー検出を保持
- **学習コスト最小**: 既存のメソッドチェーンパターンを踏襲
- **再利用性**: 全クエリ関数で共通利用可能

### 他選択肢との比較
- **Builder Pattern**: やや冗長、新たな構造体定義が必要
- **Functional Chain**: 関数型的だが、Rustエコシステムでは非標準的
- **Macro**: IDE補完が効かない懸念から除外

## 実装計画

### Phase 1: Helper Methods実装
拡張メソッドを`src/core/article/service.rs`に追加：

```rust
trait SelectStatementExt {
    fn add_url_filter(self, pattern: Option<&str>, table: impl Iden, column: impl Iden) -> Self;
    fn add_date_range(self, from: Option<DateTime<Utc>>, to: Option<DateTime<Utc>>, 
                     table: impl Iden, column: impl Iden) -> Self;
    fn add_status_filter(self, statuses: Option<&[ArticleStatus]>) -> Self;
    fn add_optional_limit(self, limit: Option<i64>) -> Self;
    fn add_source_filter(self, source: Option<&str>, table: impl Iden, column: impl Iden) -> Self;
}

impl SelectStatementExt for SelectStatement {
    // 実装詳細
}
```

### Phase 2: 既存関数のリファクタリング
以下3関数を新APIで書き換え：

1. **`search_article_url_statuses`**
   - 現在: 約40行 → 改善後: 約12行
   - URL pattern, status, limit条件を簡潔化

2. **`search_article_join_rows`** 
   - 現在: 約50行 → 改善後: 約15行
   - URL pattern, 日付範囲, status, source, limit条件を簡潔化

3. **`search_article_contents`**
   - 現在: 約35行 → 改善後: 約10行
   - URL pattern, 日付範囲, status code条件を簡潔化

### Phase 3: テスト実行・動作確認
- `cargo test`で全71テスト成功を確認
- 機能的回帰がないことを保証
- パフォーマンス影響がないことを確認

## 期待される効果

### 即座の効果
- **保守性向上**: 条件追加時のボイラープレート削除
- **可読性向上**: ビジネスロジックが明確に浮き出る
- **DRY原則達成**: 重複コードの一元化
- **開発効率**: 新規クエリ実装時間の大幅短縮

### 長期的効果
- **技術負債削減**: 冗長なパターンからの脱却
- **拡張性向上**: 新しい条件の追加が容易
- **チーム開発効率**: 統一されたパターンによる学習コスト削減

## 実装後のコード例

```rust
// Before（46行）
pub async fn search_article_url_statuses(
    query: Option<ArticleUrlStatusQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleUrlStatus>> {
    let query = query.unwrap_or_default();
    let mut select = Query::select()
        .column((ArticleLinks::Table, ArticleLinks::Url))
        .column((Articles::Table, Articles::StatusCode))
        .from(ArticleLinks::Table)
        .left_join(/* ... */)
        .to_owned();
    
    if let Some(ref url_pattern) = query.url_pattern {
        // ... 6行のボイラープレート
    }
    
    if let Some(ref statuses) = query.statuses {
        // ... 15行のボイラープレート  
    }
    
    if let Some(limit) = query.limit {
        // ... 3行のボイラープレート
    }
    // ... 以下実行部分
}

// After（15行）
pub async fn search_article_url_statuses(
    query: Option<ArticleUrlStatusQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleUrlStatus>> {
    let query = query.unwrap_or_default();
    
    let (sql, values) = Query::select()
        .column((ArticleLinks::Table, ArticleLinks::Url))
        .column((Articles::Table, Articles::StatusCode))
        .from(ArticleLinks::Table)
        .left_join(/* ... */)
        .add_url_filter(query.url_pattern.as_deref(), ArticleLinks::Table, ArticleLinks::Url)
        .add_status_filter(query.statuses.as_deref())
        .add_optional_limit(query.limit)
        .build_sqlx(PostgresQueryBuilder);
    
    // 実行部分
}
```

## 優先度
**Medium** - 機能的には完動しているが、保守性とコード品質の大幅な向上が期待される

## 関連ファイル
- `src/core/article/service.rs` - メイン修正箇所（Helper Methods実装）
- 関連テストファイル群（動作確認）

## 注意事項
- 既存APIとの互換性維持が最優先
- 全テストが通ることを必須条件とする
- パフォーマンス劣化がないことを確認

## 参考資料
- [sea-query Documentation](https://docs.rs/sea-query/)
- Issue #007: 自前QueryBuilderからsea-queryライブラリへの置き換え（前提作業）