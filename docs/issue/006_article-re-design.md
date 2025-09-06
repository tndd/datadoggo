# Article モジュールの構造改善

## 問題の概要

現在の `core/article` モジュールにおいて、ドメインオブジェクトと内部実装の境界が曖昧になっており、以下の設計上の問題が存在している。

## 現在の問題点

### 1. Article構造体の責任範囲の曖昧さ

- **問題**: `Article` 構造体が `pub status_code: i32` フィールドを公開している
- **影響**: HTTPステータスコード（200, 404等）という内部実装の詳細がユーザーに露出
- **期待**: ユーザーは「記事が取得できたか」のみを気にするべきで、HTTPレスポンスの詳細は不要

```rust
// 現状（問題のある状態）
pub struct Article {
    pub url: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status_code: i32,  // ← HTTPステータスコードが公開されている
    pub content: String,
}
```

### 2. ArticleStatusの配置ミス

- **問題**: `ArticleStatus` enum が `model.rs` に配置されているが、内部実装の詳細を含んでいる
- **詳細**: 
  - `Error(i32)` - HTTPステータスコードを含む
  - `Unprocessed` - データベース処理状況
  - `Success` - HTTPレスポンス状況
- **期待**: これらは全て内部実装として `service.rs` に隠蔽されるべき

```rust
// 現状（問題のある状態）
pub enum ArticleStatus {  // model.rsに配置されているが内部詳細
    Unprocessed,          // DB処理状況
    Success,              // HTTP 200
    Error(i32),           // HTTPステータスコード
}
```

### 3. ドメイン境界の不明確さ

**ユーザーが本来気にするべきこと（model.rs）:**
- `Article` - 取得済み記事データ（純粋なデータオブジェクト）
- `ArticleQuery` - 記事検索条件
- `search_articles` - 記事検索機能

**内部実装として隠すべきもの（service.rs）:**
- `ArticleStatus` - 処理状況やHTTPレスポンス詳細
- `ArticleInfo` - 処理状況付き軽量版
- `ArticleStorageData` - DB保存用データ
- すべてのDB操作とAPI連携

## 提案する解決策

### 1. Article構造体の純粋化

```rust
// 改善後
pub struct Article {
    pub url: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub content: String,  // status_codeを削除
}
```

### 2. ArticleStatusの内部実装化

```rust
// service.rs内で非公開として定義
enum ArticleStatus {
    Unprocessed,
    Success, 
    Error(i32),
}
```

### 3. 責任の明確な分離

**model.rs（ユーザー向けドメイン）:**
- 内部実装の詳細を一切含まない純粋なビジネスオブジェクト
- HTTPステータスコード、DB処理状況などの実装詳細は含まない

**service.rs（内部実装）:**
- すべての実装詳細（HTTP、DB、状態管理）を隠蔽
- 外部からは詳細な処理状況を知ることができない設計

## 期待される効果

1. **ドメイン境界の明確化**: ユーザーが気にするべき情報と実装詳細の分離
2. **保守性の向上**: 内部実装の変更がユーザーコードに影響しない
3. **テスタビリティの向上**: 純粋なドメインオブジェクトによる単純なテスト
4. **API設計の改善**: 実装詳細に依存しないクリーンなインターフェース

## 実装タスク

- [ ] ArticleStatusをmodel.rsからservice.rsに移設
- [ ] Article構造体からstatus_codeフィールドを削除  
- [ ] Articleのget_article_statusとis_errorメソッドを削除
- [ ] mod.rsからArticleStatusのエクスポートを削除
- [ ] 影響を受けるテストとコードを修正
- [ ] 新しい構造でのテスト実装

## 備考

この変更により、`core/article` モジュールは真のドメイン駆動設計に沿った構造となり、ユーザーは実装の詳細に煩わされることなく記事データを扱えるようになる。