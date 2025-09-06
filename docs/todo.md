# Issue
- [ ] search_backlog_articles_light, search_backlog_article_linksの類似
- [ ] Article構造体のに意図しないOption指定がある問題
- [ ] Article構造体がArticleStatusを持っていない問題

# Todo

---

# Done
## rss_linkという具象的すぎる名前の改名
- 本来これはスクレイプする前の待機リストの役割がある
- つまり、むしろarticle_linkという名前での方が正しい。
	- rssのリンクというわけでもないから
- さらにこうした方が、この情報源の由来がrssやそうでないかに囚われなくできる
- となると追加で情報の由来を表すフィールドを追加するのもあり

## 自動cargo fmt
- コミット時に自動で`cargo fmt`を実行する

## article再編
- 実装が嵩張ってきたので、articleディレクトリを切りファイル分割

## dataディレクトリ移動
- プロジェクトルートに移すべきかもしれないので検討

## sqlディレクトリの作成
- fixtures, migrationsという二つのsqlファイルのディレクトリが存在してる
- これらをsqlディレクトリにを作って纏める
- だがこの二つのディレクトリはsqlxの仕様上、位置を変更することができないかもしれないので注意
- fixtures内部についてもドメインごとにsqlファイルの分割を行う
