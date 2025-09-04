<!-- WARN: rss_link->article_linkへの改名が完了してから文書更新 -->

# `article` モジュールをサブモジュール化してファイル分割する

- 種別: リファクタリング
- 優先度: 高
- 影響範囲: `src/domain/article.rs`, テスト, `src/app/workflow.rs`（呼び出し元）

## 背景 / 課題

`src/domain/article.rs` は、ドメイン型（Article, ArticleLight, ArticleContent, ArticleStatus, ArticleView）、DBアクセス（保存・検索）、Firecrawl連携（取得）、ヘルパ・テストが一つに集中しており肥大化している。可読性・保守性・コンパイル時間・テストの見通しを改善するため、責務単位で分割する。

## 目標

- 責務に応じてファイルを分割し、公開APIは現状互換（モジュールの `pub use` による再エクスポート）
- データ型・振る舞い（トレイト実装）と、副作用コード（DB, HTTP）を分離

## 提案構成

- `src/domain/article/`
  - `mod.rs`（モジュールツリー定義と `pub use`）
  - `model.rs`（Article, ArticleLight, ArticleStatus, ArticleView など純粋な型と実装）
  - `content.rs`（ArticleContent とその保存/検索）
  - `repo.rs`（Article/ArticleLight の検索クエリ）
  - `service.rs`（Firecrawl 統合: get_article_content / get_article_content_with_client）
  - `tests/`（ユニット・統合テスト分割、または各ファイル内テストの維持）

既存の `src/domain/article.rs` は薄いファサードにし、最終的に各モジュールへの委譲 + `pub use` のみを残す。

## 実装計画

1) ディレクトリ作成と責務ごとの搬出
- `src/domain/article/` を作成
- `model.rs`, `content.rs`, `repo.rs`, `service.rs` を追加し、既存コードを移動
- 公開関数/型名は維持し `mod.rs` で `pub use` を提供

2) 参照更新
- 外部からの `use` は現状維持（re-export前提）
- 内部参照はファイル分割に合わせて修正

3) テスト整理
- 大きな統合テストは `repo.rs` や `service.rs` に移設、または `tests/` に分割
- `#[sqlx::test]` の fixtures 相対パスは変更後の階層に合わせて要確認

4) ビルド/CI
- 分割後に `cargo test` を通す

## タスク（チェックリスト）
- [ ] `article/` ディレクトリ作成・`mod.rs` 追加
- [ ] `model.rs` に純粋ドメイン型とトレイト実装を移動
- [ ] `content.rs` に ArticleContent + 保存/検索を移動
- [ ] `repo.rs` に Article/ArticleLight の検索を移動
- [ ] `service.rs` に Firecrawl 連携関数を移動
- [ ] 参照・use 更新（re-exportで外部互換）
- [ ] テスト移設/分割と fixtures パスの確認
- [ ] `cargo test` 総合確認

## 受け入れ条件
- 外部モジュールからの関数・型の呼び出しシグネチャは互換（または最小限の差分はPRで明示）
- すべてのテストがグリーン
- 各ファイルの責務が分かれ、LOCが適正化

## リスク / メモ
- ファイル分割により `#[cfg(test)]` と fixtures 相対パスが壊れるリスク
- 将来、`service` のHTTPクライアント抽象を強化する余地あり