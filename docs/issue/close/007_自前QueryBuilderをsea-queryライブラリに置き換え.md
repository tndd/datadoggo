# Issue #007: 自前QueryBuilderをsea-queryライブラリに置き換え

## 背景・動機

現在の実装では自前でQueryBuilder構造体を実装しているが、これは車輪の再発明である。Rustエコシステムには成熟したクエリビルダーライブラリが存在するため、業界標準のライブラリに移行すべき。

## 選定したソリューション: sea-query

### 選定理由
- **SQLx完全互換**: 既存のSQLx環境に完全統合可能
- **成熟性**: SeaORMの基盤として実績のあるライブラリ
- **型安全性**: Rustの型システムを活用した安全な動的クエリ構築
- **豊富な機能**: WHERE句、JOIN、ORDER BY、LIMIT等すべてサポート
- **アクティブメンテナンス**: 2025年も継続的に更新中

## 現在の進捗状況

### ✅ 完了済み
1. **依存関係追加**: `Cargo.toml`に`sea-query`と`sea-query-binder`を追加
2. **自前QueryBuilder削除**: 自前実装を削除し、sea-queryインポート追加
3. **テーブル定義追加**: `ArticleLinks`と`Articles` enumを定義
4. **関数の骨格変更**: 3つの主要関数を部分的に書き換え
   - `search_article_url_statuses()`
   - `search_article_join_rows()`  
   - `search_article_contents()`

### 🚧 実装途中の問題

#### コンパイルエラーが残存
```
error[E0599]: no method named `build_sqlx` found for struct `sea_query::SelectStatement`
```

#### 修正が必要な箇所
1. **API使用法の修正**: `build_sqlx` → 正しいAPI
2. **条件処理**: `and_where` → `cond_where`
3. **PostgreSQL拡張**: `ilike`メソッドのためのトレイトインポート

## 残りの作業

### Phase 1: コンパイルエラー修正
- [ ] `build_sqlx`を正しいsea-query-binder APIに修正
- [ ] `PgExpr`トレイトのインポート確認
- [ ] `Cond::any()`の正しい使用法に修正

### Phase 2: 動作確認
- [ ] `cargo test`でテスト実行
- [ ] 回帰がないことを確認
- [ ] パフォーマンステスト

### Phase 3: コード品質向上
- [ ] より表現豊かなクエリ構築パターンの採用
- [ ] エラーハンドリングの改善
- [ ] ドキュメント更新

## 技術的詳細

### 現在の依存関係
```toml
sea-query = { version = "0.31", features = ["backend-postgres", "with-chrono"] }
sea-query-binder = { version = "0.7", features = ["sqlx-postgres", "with-chrono"] }
```

### 修正が必要なコード例
```rust
// 現在（エラー）
let (sql, values) = select.build_sqlx(PostgresQueryBuilder);

// 修正後（推定）
let (sql, values) = select.build_sqlx(PostgresQueryBuilder);
// または
let sql = select.to_string(PostgresQueryBuilder);
// その後、適切なバインディング処理
```

## 期待される効果

### 即座の効果
- **保守性向上**: 業界標準ライブラリ使用による長期的サポート
- **バグ削減**: 実績のあるライブラリによる安定性向上
- **開発効率**: 豊富なドキュメントとコミュニティサポート

### 長期的効果
- **技術負債削減**: 自前実装からライブラリへの移行
- **機能拡張性**: sea-queryの高度な機能を活用可能
- **チーム開発効率**: 標準的なAPIによる学習コスト低減

## 優先度
**High** - 現在コンパイルエラーが発生しているため、早急な修正が必要

## 関連ファイル
- `src/core/article/service.rs` - メイン修正箇所
- `Cargo.toml` - 依存関係設定
- 関連テストファイル群

## 注意事項
- セッション中にファイル変更が困難な状況が発生
- 次回作業時は、まず`cargo check`でコンパイル状況を確認
- sea-query-binderの正しいAPI使用法を要調査

## 参考資料
- [sea-query Documentation](https://docs.rs/sea-query/)
- [sea-query-binder Documentation](https://docs.rs/sea-query-binder/)
- [SeaORM Book](https://www.sea-ql.org/SeaORM/)