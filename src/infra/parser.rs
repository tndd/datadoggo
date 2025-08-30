use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};

/// 文字列を日付型に変換するヘルパー関数
///
/// `dateparser`クレートを利用して、様々な形式の日付文字列を解析し、
/// `DateTime<Utc>`型に変換する。
///
/// **この関数の意義**
/// `dataparser::parse`で行われないwith_timezoneでUTCへの変換を行なってる。
/// また`dataparser::parse`で対応できない文字列が来た場合でも、問題をこの関数で吸収できる。
///
/// # サポート形式の例
/// - "2025-01-15"
/// - "2025-01-15T10:00:00Z"
/// - "Sun, 10 Aug 2025 12:00:00 +0000"
///
/// # 引数
/// - `date_str`: 解析対象の日付文字列
///
/// # 戻り値
/// - `Ok(DateTime<Utc>)`: 解析が成功した場合
/// - `Err(anyhow::Error)`: 解析に失敗した場合
pub fn parse_date(date_str: &str) -> Result<DateTime<Utc>> {
    // `dateparser`はタイムゾーンを持つ`DateTime`を返すため、UTCに変換する
    match dateparser::parse(date_str) {
        Ok(dt) => Ok(dt.with_timezone(&Utc)),
        Err(_) => Err(anyhow!("不正な日付形式: {}", date_str)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    // `parse_date_string`関数の基本的なテスト
    #[test]
    fn test_parse_common_date_formats() {
        // ISO 8601 / RFC 3339
        let rfc3339 = "2025-08-10T12:30:00Z";
        let expected_rfc3339 = Utc.with_ymd_and_hms(2025, 8, 10, 12, 30, 0).unwrap();
        assert_eq!(parse_date(rfc3339).unwrap(), expected_rfc3339);

        // RFC 2822 (RSSで一般的)
        let rfc2822 = "Sun, 10 Aug 2025 12:30:00 +0000";
        assert_eq!(parse_date(rfc2822).unwrap(), expected_rfc3339);

        // YYYY-MM-DD（dateparserは現在時刻で補完するため、日付のみをチェック）
        let ymd = "2025-08-10";
        let parsed_ymd = parse_date(ymd).unwrap();
        assert_eq!(
            parsed_ymd.date_naive(),
            chrono::NaiveDate::from_ymd_opt(2025, 8, 10).unwrap(),
            "日付部分が期待と異なります"
        );
    }

    // タイムゾーン付きの日付文字列のテスト
    #[test]
    fn test_parse_with_timezones() {
        // JST (+09:00)
        let jst_str = "2025-08-10T21:30:00+09:00";
        let expected_utc = Utc.with_ymd_and_hms(2025, 8, 10, 12, 30, 0).unwrap();
        assert_eq!(parse_date(jst_str).unwrap(), expected_utc);

        // PST (-08:00)
        let pst_str = "2025-08-10T04:30:00-08:00";
        assert_eq!(parse_date(pst_str).unwrap(), expected_utc);
    }

    // 不正な日付形式のテスト
    #[test]
    fn test_parse_invalid_formats() {
        assert!(parse_date("invalid-date").is_err());
        assert!(parse_date("2025-13-40").is_err()); // 不正な月日
        assert!(parse_date("").is_err()); // 空文字列
    }

    // mock/rss/*.rss ファイルの日付形式を模倣したテスト
    #[test]
    fn test_parse_from_mock_rss_files() {
        // bbc.rss: "Sun, 27 Jul 2025 07:36:19 GMT"の形式
        let bbc_date = "Sun, 27 Jul 2025 07:36:19 GMT";
        let expected_bbc = Utc.with_ymd_and_hms(2025, 7, 27, 7, 36, 19).unwrap();
        assert_eq!(parse_date(bbc_date).unwrap(), expected_bbc);

        // cbs.rss: "Sun, 27 Jul 2025 03:25:12 -0400"の形式（タイムゾーンオフセット付き）
        let cbs_date = "Sun, 27 Jul 2025 03:25:12 -0400";
        let expected_cbs = Utc.with_ymd_and_hms(2025, 7, 27, 7, 25, 12).unwrap(); // -0400 = UTC+4時間
        assert_eq!(parse_date(cbs_date).unwrap(), expected_cbs);

        // guardian.rss: "Wed, 23 Jul 2025 04:00:42 GMT"の形式
        let guardian_date = "Wed, 23 Jul 2025 04:00:42 GMT";
        let expected_guardian = Utc.with_ymd_and_hms(2025, 7, 23, 4, 0, 42).unwrap();
        assert_eq!(parse_date(guardian_date).unwrap(), expected_guardian);

        // その他のRFC 2822形式パターンもテスト
        let rfc2822_variations = [
            (
                "Sat, 26 Jul 2025 18:02:24 GMT",
                Utc.with_ymd_and_hms(2025, 7, 26, 18, 2, 24).unwrap(),
            ),
            (
                "Sun, 27 Jul 2025 03:38:00 -0400",
                Utc.with_ymd_and_hms(2025, 7, 27, 7, 38, 0).unwrap(),
            ),
            (
                "Sun, 27 Jul 2025 02:01:14 GMT",
                Utc.with_ymd_and_hms(2025, 7, 27, 2, 1, 14).unwrap(),
            ),
        ];

        for (date_str, expected) in &rfc2822_variations {
            assert_eq!(
                parse_date(date_str).unwrap(),
                *expected,
                "日付文字列 '{}' のパースが期待と異なります",
                date_str
            );
        }
    }
}
