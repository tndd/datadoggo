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
| Field      | Type                  |
| ---------- | --------------------- |
| url        | String                |
| title      | String                |
| pub_date   | DateTime<Utc>         |
| updated_at | Option<DateTime<Utc>> |
| content    | Option<String>        |

---

注意点:
- status codeの削除
  - ユーザーが`Article`を取得している時点で、エラーなど情報として成立していない情報が取得されることはありえないため削除
- optionalの削除
  - こちらについても、データの欠損はありえないためoptionalは削除

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
ユーザーが`Article`を取得する際に使う関数であるため、mod.rs側に移動させる。

---

注意点:
- ArticleQuery変更に伴うsqlの修正
  - statusがクエリから消し去られたに伴い、この関数のsqlやその他の処理に関しても修正が必要となる

## [service.rs](../../src/core/article/service.rs)
その他のドメイン領域から外れたdirtyな処理を集めたファイル。\
mod.rsとは従属的な関係となる。

# 要望
- 過度な抽象化は控えること
- なるべく簡潔な実装を行い可読性を上げること
- 後方互換性を無視して作り直すこと