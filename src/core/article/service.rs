// 共有ユーティリティをここに集約（article配下でのみ共有）
use super::model::ArticleStatus;

/// 内部実装：statuses指定の正規化を行う
/// queryのSQLバインド補助。記事検索系でのみ利用するため公開は限定。
pub(super) fn normalize_statuses(
    statuses: Option<&[ArticleStatus]>,
) -> (bool, bool, bool, Option<Vec<i32>>) {
    let mut apply = false;
    let mut has_unprocessed = false;
    let mut has_success = false;
    let mut errors: Vec<i32> = Vec::new();

    if let Some(list) = statuses {
        for s in list {
            apply = true;
            match s {
                ArticleStatus::Unprocessed => has_unprocessed = true,
                ArticleStatus::Success => has_success = true,
                ArticleStatus::Error(code) => errors.push(*code),
            }
        }
    }

    let error_codes = if errors.is_empty() {
        None
    } else {
        Some(errors)
    };
    (apply, has_unprocessed, has_success, error_codes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_statuses() {
        let (apply, unp, ok, errs) = normalize_statuses(Some(&[
            ArticleStatus::Unprocessed,
            ArticleStatus::Success,
            ArticleStatus::Error(404),
            ArticleStatus::Error(500),
        ]));
        assert!(apply);
        assert!(unp);
        assert!(ok);
        assert_eq!(errs.unwrap(), vec![404, 500]);
    }
}
