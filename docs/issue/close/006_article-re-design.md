# Article モジュールの構造改善
[mod.rs](../../src/core/article/mod.rs)にユーザー側が使うものを集め、[service.rs](../../src/core/article/service.rs)に外部からの情報取得や関数を集める棲み分けを行いたいと考えている。

# 修正対象
## [mod.rs](../../src/core/article/mod.rs)
中間データやdbのテーブル定義に引っ張られない情報が集まるファイル。\
ユーザーが直接見る

### Article
ユーザー側が実際に取り扱う情報モデル。

---

テーブル定義
| Field      | Type          |
| ---------- | ------------- |
| url        | String        |
| title      | String        |
| pub_date   | DateTime<Utc> |
| updated_at | DateTime<Utc> |
| content    | String        |

---

注意点:
- status codeの削除
  - ユーザーが`Article`を取得したいと考えている時点で、エラーや未取得の記事を返すことは理屈としておかしいから
  - もしエラーや未取得の記事という特殊な情報が欲しいならば、後述の`ArticleUrlStatus`を使用すればいい
- optionalの削除
  - こちらについても、データの欠損はありえないためoptionalは削除
- ドメインモデルとしての純化による影響
  - db側では依然、joinされた結果のoptionalな結果が返ってはくるが、それはこのモデルとは全く別物であることには留意すること
  - そういう雑多なレスポンスがフィルタリングされ純化されたものがこのArticleモデルとして使われるという想定

### ArticleQuery
ユーザーが`Article`を取得する際に使用するクエリモデル。

---

テーブル定義:
| Field         | Type                  |
| ------------- | --------------------- |
| link_pattern  | Option<String>        |
| pub_date_from | Option<DateTime<Utc>> |
| pub_date_to   | Option<DateTime<Utc>> |
| limit         | Option<i64>           |

---

注意点:
- article_statusの削除
  - こちらについても、ユーザーがエラーや未取得といったドメインモデルの体をなしてない情報を取得することはありえないから
  - ただしservice側のqueryに関してはstatusの指定が必要になるだろうが、やはりここでは不要


### search_articles()
- ユーザーが`Article`を取得する際に使う関数であるため、mod.rs側に移動させる
- 検索対象はstatus_codeは200であることを前提とする

---

注意点:
- ArticleQuery変更に伴うsqlの修正
  - statusがクエリから消し去られたに伴い、この関数のsqlやその他の処理に関しても修正が必要となる

## [service.rs](../../src/core/article/service.rs)
その他のドメイン領域から外れたdirtyな処理を集めたファイル。\
mod.rsとは従属的な関係となる。

### ArticleStatus
- ドメイン層においてstatus codeがお役御免になったため、これはservice.rsへ移動

# 削除対象
## ArticleMetadata
- ユーザーがメタデータだけ欲しいという場面は今のところ考えられないため不要
- 必要ならば代替品として後述の`ArticleUrlStatus`を使用すること

## ArticleContentQuery
- これは`ArticleContent`というモデルの取得を目的として作られたクエリ構造体
- だが、ArticleContent単体で情報を取得するというケースはない
  - それはユーザー側から見てもそう
  - そしてservice側から見ても、このArticleContentはArticleLinkテーブルとjoinして使用されうるものだからだ

# 追加対象(service.rs)
## ArticleUrlStatus
- どのurlがどういうステータスを持っているかを確認するための軽量な構造体
- 想定している用途は、エラーや未実行のurlを割り出し取得するため

---

テーブル定義:
| Field       | Type                  |
| ----------- | --------------------- |
| url         | String                |
| status_code | Option<i32>           |
| pub_date    | DateTime<Utc>         |

---

注意点:
- このArticleUrlStatus追加に伴い、その取得のためのクエリと関数とテストの実装が必要となる
- 失敗URL抽出や優先度付けの判断材料として `pub_date` を保持する（既存テーブル `article_links.pub_date` の値）

## ArticleJoinRow（廃止）
この中間体は廃止。`search_articles` はDB側で成功行のみを絞り込み、`Article` を直接 `FromRow` で受け取る。

---

テーブル定義:
| Field       | Type                  |
| ----------- | --------------------- |
| url         | String                |
| title       | String                |
| pub_date    | DateTime<Utc>         |
| source      | String                |
| timestamp   | Option<DateTime<Utc>> |
| status_code | Option<i32>           |
| content     | Option<String>        |

---

注意点:
- ドメイン`Article`への変換機能が必要となる
  - timestampは`Article`における`update_at`
  - sourceは`Article`には変換されない

## ArticleJoinRowQuery（廃止）
`ArticleJoinRow` 廃止に伴い不要。期間・パターン・件数等の条件は `ArticleQuery` に集約。

---

テーブル定義:
| Field         | Type                  |
| ------------- | --------------------- |
| link_pattern  | Option<String>        |
| pub_date_from | Option<DateTime<Utc>> |
| pub_date_to   | Option<DateTime<Utc>> |
| status_codes  | Option<Vec<i32>>      |
| source        | Option<String>        |
| limit         | Option<i64>           |

---

## search_article_join_rows()（廃止）
低レベルJOINの取得関数は削除。`search_articles` が直接SQLで `Article` を返す。

# 要望
- 過度な抽象化は控えること
- なるべく簡潔な実装を行い可読性を上げること
- 後方互換性は一切考慮せず、新たに作り直すこと
