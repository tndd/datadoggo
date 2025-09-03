# プロジェクト概要
このプロジェクトはwebからニュース等のを集め、保存・分析を行うこと。
もっと言うとwebの主要な情報を監視すること。

# 情報源
- 公開rssフィードによるニュース監視
- blueskyのrssフィード
- google newsのrssフィード
- 指定サイトへの直接スクレイピング

## RSS
- bbc
- cbs
- the_guardian
- cnbc
- yahoo_japan
- rss_club
- grand_fleet
- japan_government
- ...

## Bluesky
WIP

## Google News
WIP

## Scrape
- BIS（国際決済銀行） - 四半期報告書、年次経済報告書で中銀政策の知的枠組みを提供
- Federal Reserve System - FOMC議事録、経済分析、Beige Bookなど市場を動かす情報発信
- IMF - World Economic Outlook、国別審査報告書で各国政策を「採点」
- ECB（欧州中央銀行） - 金融政策決定の詳細な説明、研究論文の大量発信
- World Bank - 開発報告書、データベース公開で「開発」の定義を独占
- BlackRock - Investment Instituteのレポートで市場認識を形成
- Bank of England - 金融安定報告書、インフレ報告書で政策論議を主導
- OECD - 経済見通し、政策提言で先進国スタンダードを設定
- WEF（世界経済フォーラム） - Global Risks Report、ダボス会議で議題設定
- CFR（外交問題評議会） - Foreign Affairs誌、CFR報告書で米国外交思想を形成
- ...

# Workflow
## rss
1. feedから対象のrssフィードへのリンクを取得
2. rssフィードからニュースのリンク(`rss_link`)を取得
    - 取得した情報は`rss_link`という構造体にしてDBに保存
    - `rss_link`は、`article`の進捗管理のアンカーとしても機能してる
	- もし`rss_link`に`article`が紐ついてない場合は、記事が未取得であるという判断を行う
3. linkからニュースの内容(`article`)を取得
	- urlから取得した記事は`article`という構造体として保存する
	- `article`には正常に記事が保存されたかどうかが記録されてる`status_code`がある
	- `status_code`が200ではない場合、記事再取得の対象となる

## bluesky
## google news
## scrape
