use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ユーザー側が実際に取り扱う情報モデル（ドメイン向け）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub url: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub content: String,
}

// ユーザーがArticleを取得する際に使用するクエリモデル（ドメイン向け）
#[derive(Debug, Default)]
pub struct ArticleQuery {
    pub link_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}

// 記事の処理状態を表現するenum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArticleStatus {
    /// 記事が未処理（articleテーブルに存在しない）
    Unprocessed,
    /// 記事が正常に取得済み（status_code = 200）
    Success,
    /// 記事の取得にエラーが発生（status_code != 200）
    Error(i32),
}

// どのurlがどういうステータスを持っているかを確認するための軽量な構造体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleUrlStatus {
    pub url: String,
    pub status_code: Option<i32>,
}

// ArticleUrlStatusを取得する際に使用するクエリモデル
#[derive(Debug, Default)]
pub struct ArticleUrlStatusQuery {
    pub url_pattern: Option<String>,
    pub statuses: Option<Vec<ArticleStatus>>,
    pub limit: Option<i64>,
}

// ArticleLinkとArticleのJOIN結果をそのまま受け取るDB用の構造体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleJoinRow {
    pub url: String,
    pub title: String,
    pub pub_date: DateTime<Utc>,
    pub source: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub status_code: Option<i32>,
    pub content: Option<String>,
}

// ArticleJoinRowを取得する際に使用するクエリモデル
#[derive(Debug, Default)]
pub struct ArticleJoinRowQuery {
    pub link_pattern: Option<String>,
    pub pub_date_from: Option<DateTime<Utc>>,
    pub pub_date_to: Option<DateTime<Utc>>,
    pub statuses: Option<Vec<ArticleStatus>>,
    pub source: Option<String>,
    pub limit: Option<i64>,
}

// 記事内容の構造体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleContent {
    pub url: String,
    pub timestamp: DateTime<Utc>, // (updated_at)
    pub status_code: i32,
    pub content: String,
}

#[cfg(test)]
mod tests {
    //! このモジュールは `core/article/model.rs` に定義される型（`Article`,
    //! `ArticleStatus`, 各種Query, `ArticleJoinRow`, `ArticleContent`）の
    //! シリアライズ/デシリアライズ仕様と `Default` の初期値契約を検証する。
    //!
    //! 方針:
    //! - JSON表現は将来の変更に強い「意味」一致で検証する（時刻はRFC3339再パースで比較）。
    //! - Query型の`Default`は全フィルタ未設定（None）であることを保証する。
    //! - enum `ArticleStatus` はSerdeのexternal taggingに準拠していることを確認する。

    use super::*;
    use serde_json::{self, Value};

    // tests直下にヘルパーを定義し、helperモジュールでそのヘルパー自体もテストする
    /// RFC3339文字列をUTC時刻に変換するユーティリティ。
    ///
    /// 例: `"2025-01-02T03:04:05Z"` → `DateTime<Utc>`
    ///
    /// - 入力: RFC3339に準拠した文字列
    /// - 出力: `Utc` タイムゾーンに正規化した `DateTime<Utc>`
    fn ts(s: &str) -> DateTime<Utc> {
        // 例: "2025-01-02T03:04:05Z"
        chrono::DateTime::parse_from_rfc3339(s)
            .expect("不正なRFC3339形式")
            .with_timezone(&Utc)
    }

    /// 任意の`Serialize`を`serde_json::Value`へ変換するヘルパー。
    ///
    /// - 目的: テスト内でフィールドを直接参照しやすくする。
    /// - 非ゴール: パフォーマンス検証やスキーマ定義の生成は対象外。
    fn to_json_value<T: serde::Serialize>(v: &T) -> Value {
        serde_json::to_value(v).expect("JSON変換に失敗")
    }

    /// helperモジュール: 上記ヘルパー自体の基本動作を検証する。
    mod helper {
        use super::*;

        /// テスト目的
        /// - RFC3339文字列がUTC(+00:00)として解釈されることを確認する。
        ///
        /// 検証観点
        /// - `to_rfc3339()` の出力が `+00:00` で正規化される。
        ///
        /// 非ゴール
        /// - 曖昧なローカル時刻や閏秒の扱いは対象外。
        #[test]
        fn parses_rfc3339_to_utc() {
            let dt = ts("2020-05-01T12:34:56Z");
            // UTC(+00:00)として解釈されること
            assert_eq!(dt.to_rfc3339(), "2020-05-01T12:34:56+00:00");
        }

        /// テスト目的
        /// - `to_json_value` がスカラー値を正しく `Value` に変換すること。
        ///
        /// 検証観点
        /// - `123` → `Value::from(123)`。
        ///
        /// 非ゴール
        /// - 浮動小数点や巨大整数の丸め動作の精査は対象外。
        #[test]
        fn to_json_value_converts_scalar() {
            let v = to_json_value(&123);
            assert_eq!(v, Value::from(123));
        }
    }

    // 以降、対象型ごとにサブモジュールへ分類（helper → online → {対象名} の順）

    /// `ArticleStatus` に関するテスト群。
    mod article_status {
        use super::*;
        use serde_json::{self, json, Value};

        /// テスト目的
        /// - `ArticleStatus` のユニットバリアント（`Unprocessed`/`Success`）が
        ///   Serdeのexternal taggingで文字列としてシリアライズ・デシリアライズ
        ///   されることを確認する。
        ///
        /// 検証観点
        /// - 直列化: `Unprocessed` → `"Unprocessed"`、`Success` → `"Success"`
        /// - 逆直列化: `"Success"` → `ArticleStatus::Success`
        ///
        /// 非ゴール
        /// - 大文字小文字のゆらぎや未知値の扱いはこのテストでは扱わない。
        #[test]
        fn unit_variants() {
            let v_unprocessed = to_json_value(&ArticleStatus::Unprocessed);
            let v_success = to_json_value(&ArticleStatus::Success);
            assert_eq!(v_unprocessed, Value::from("Unprocessed"));
            assert_eq!(v_success, Value::from("Success"));

            let s = "\"Success\""; // JSON文字列
            let status: ArticleStatus = serde_json::from_str(s).expect("deserialize失敗");
            match status {
                ArticleStatus::Success => {}
                _ => panic!("Successとして復元されるべき"),
            }
        }

        /// テスト目的
        /// - `ArticleStatus::Error(i32)` のタプルバリアントが
        ///   `{ "Error": <code> }` 形式で（external tagging）シリアライズ/デシリアライズ
        ///   されることを確認する。
        ///
        /// 検証観点
        /// - 直列化: `Error(404)` → `{ "Error": 404 }`
        /// - 逆直列化: `{ "Error": 500 }` → `Error(500)`
        ///
        /// 非ゴール
        /// - エラーコードの意味付けや範囲チェックは他レイヤに委譲。
        #[test]
        fn error_variant() {
            let v = to_json_value(&ArticleStatus::Error(404));
            assert_eq!(v, json!({ "Error": 404 }));

            let s = "{\"Error\":500}";
            let status: ArticleStatus = serde_json::from_str(s).expect("deserialize失敗");
            match status {
                ArticleStatus::Error(code) => assert_eq!(code, 500),
                _ => panic!("Error(500)として復元されるべき"),
            }
        }
    }

    /// Queryモデルの `Default` 契約に関するテスト。
    mod queries {
        use super::*;

        /// テスト目的
        /// - Queryモデル（`ArticleQuery`, `ArticleUrlStatusQuery`, `ArticleJoinRowQuery`）の
        ///   `Default` 実装が「全て未設定（None）」から開始するという契約を保証する。
        ///
        /// 背景
        /// - デフォルト値が暗黙のフィルタを生まないことで、呼び出し側が
        ///   意図しない絞り込みを避けるため。
        ///
        /// 非ゴール
        /// - 値設定後の組み合わせロジックはリポジトリ層のテストで担保する。
        #[test]
        fn default_are_none() {
            let q = ArticleQuery::default();
            assert!(q.link_pattern.is_none());
            assert!(q.pub_date_from.is_none());
            assert!(q.pub_date_to.is_none());
            assert!(q.limit.is_none());

            let qs = ArticleUrlStatusQuery::default();
            assert!(qs.url_pattern.is_none());
            assert!(qs.statuses.is_none());
            assert!(qs.limit.is_none());

            let jq = ArticleJoinRowQuery::default();
            assert!(jq.link_pattern.is_none());
            assert!(jq.pub_date_from.is_none());
            assert!(jq.pub_date_to.is_none());
            assert!(jq.statuses.is_none());
            assert!(jq.source.is_none());
            assert!(jq.limit.is_none());
        }
    }

    /// `Article` のシリアライズ仕様のテスト。
    mod article {
        use super::*;
        use serde_json::Value;

        /// テスト目的
        /// - `Article` 構造体がJSONへシリアライズされる際に、必須フィールドが
        ///   正しく出力され、日時がRFC3339文字列で表現されることを確認する。
        ///
        /// 検証観点
        /// - `url`/`title`/`content` の存在と値
        /// - `pub_date`/`updated_at` はRFC3339再パースで元のUTCと同値
        ///   （`"Z"` と `"+00:00"` の表記差を吸収）
        ///
        /// 非ゴール
        /// - タイトルや本文の正規化・サマリ生成などの上位ロジックは対象外。
        #[test]
        fn serializes_expected_fields() {
            let article = Article {
                url: "https://example.com/a".to_string(),
                title: "hello".to_string(),
                pub_date: ts("2020-05-01T12:34:56Z"),
                updated_at: ts("2020-05-02T00:00:00Z"),
                content: "content...".to_string(),
            };

            let v = to_json_value(&article);
            assert_eq!(v["url"], Value::from("https://example.com/a"));
            assert_eq!(v["title"], Value::from("hello"));
            assert_eq!(v["content"], Value::from("content..."));

            let pub_date_s = v["pub_date"].as_str().expect("pub_dateは文字列");
            let updated_at_s = v["updated_at"].as_str().expect("updated_atは文字列");
            let pub_date_parsed = chrono::DateTime::parse_from_rfc3339(pub_date_s)
                .expect("pub_dateはRFC3339")
                .with_timezone(&Utc);
            let updated_at_parsed = chrono::DateTime::parse_from_rfc3339(updated_at_s)
                .expect("updated_atはRFC3339")
                .with_timezone(&Utc);
            assert_eq!(pub_date_parsed, ts("2020-05-01T12:34:56Z"));
            assert_eq!(updated_at_parsed, ts("2020-05-02T00:00:00Z"));
        }
    }

    /// `ArticleJoinRow` のシリアライズ仕様のテスト。
    mod article_join_row {
        use super::*;
        use serde_json::Value;

        /// テスト目的
        /// - `ArticleJoinRow` の `timestamp: None` が JSON `null` として出力される
        ///   こと、`status_code`/`content` が適切に出力されることを確認する。
        ///
        /// 背景
        /// - JOIN結果の一部列（コンテンツ更新時刻など）が存在しないケースを
        ///   破壊的変更なく表現するために `Option` を採用。
        #[test]
        fn serializes_null_timestamp() {
            let row = ArticleJoinRow {
                url: "https://example.com/a".to_string(),
                title: "hello".to_string(),
                pub_date: ts("2020-05-01T12:34:56Z"),
                source: "feedA".to_string(),
                timestamp: None,
                status_code: Some(200),
                content: Some("ok".to_string()),
            };

            let v = to_json_value(&row);
            assert_eq!(v["timestamp"], Value::Null);
            assert_eq!(v["status_code"], Value::from(200));
            assert_eq!(v["content"], Value::from("ok"));
        }
    }

    /// `ArticleContent` の往復シリアライズテスト。
    mod article_content {
        use super::*;

        /// テスト目的
        /// - `ArticleContent` の JSON 往復（serialize → deserialize）で、
        ///   すべてのフィールドが完全一致することを確認する。
        ///
        /// 検証観点
        /// - URL・時刻（UTC）・ステータスコード・本文の一致
        ///
        /// 非ゴール
        /// - 本文の圧縮・暗号化など表現最適化は別責務。
        #[test]
        fn roundtrip() {
            let src = ArticleContent {
                url: "https://example.com/a".to_string(),
                timestamp: ts("2021-01-01T00:00:01Z"),
                status_code: 200,
                content: "BODY".to_string(),
            };

            let s = serde_json::to_string(&src).expect("serialize失敗");
            let dst: ArticleContent = serde_json::from_str(&s).expect("deserialize失敗");

            assert_eq!(dst.url, src.url);
            assert_eq!(dst.timestamp, src.timestamp);
            assert_eq!(dst.status_code, src.status_code);
            assert_eq!(dst.content, src.content);
        }
    }
}
