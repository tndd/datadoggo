use sha2::{Digest, Sha256};

/// 文字列からSHA256ベースのハッシュ値を計算する
///
/// # Arguments
/// * `input` - ハッシュ計算対象の文字列
/// * `length` - 出力するハッシュの長さ（最大64文字）
///
/// # Returns
/// 指定された長さに制限されたSHA256ハッシュ文字列（16進数）
pub fn calc_hash(input: &str, length: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash_bytes = hasher.finalize();
    let hash_hex = format!("{:x}", hash_bytes);

    // SHA256は64文字の16進数文字列を生成するため、指定された長さでトリミング
    let max_length = length.min(hash_hex.len());
    hash_hex[..max_length].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calc_hash() {
        let input1 = "test string 1";
        let input2 = "test string 2";
        let rss_url = "https://example.com/rss.xml";

        // デフォルト長さ（6）でのテスト
        let hash_default = calc_hash(input1, 6);

        // 異なる長さでのテスト
        let hash_3 = calc_hash(input1, 3);
        let hash_6 = calc_hash(input1, 6);
        let hash_10 = calc_hash(input1, 10);

        // RSS URLのテスト
        let rss_hash = calc_hash(rss_url, 6);

        println!("input1のハッシュ (6): {} (長さ: {})", hash_6, hash_6.len());
        println!(
            "RSS URLのハッシュ (6): {} (長さ: {})",
            rss_hash,
            rss_hash.len()
        );

        // 指定した長さ以下であることを確認
        assert!(hash_default.len() <= 6);
        assert!(hash_3.len() <= 3);
        assert!(hash_6.len() <= 6);
        assert!(hash_10.len() <= 10);

        // 空でないことを確認
        assert!(hash_default.len() > 0);
        assert!(hash_3.len() > 0);
        assert!(hash_6.len() > 0);
        assert!(hash_10.len() > 0);

        // 異なる入力は異なるハッシュを生成
        let hash1_6 = calc_hash(input1, 6);
        let hash2_6 = calc_hash(input2, 6);
        assert_ne!(hash1_6, hash2_6);

        // 同じ入力と長さは同じハッシュを生成
        assert_eq!(hash1_6, calc_hash(input1, 6));

        // デフォルト値と明示的な6は同じ結果
        assert_eq!(hash_default, hash_6);
    }

    #[test]
    fn test_calc_hash_consistency() {
        let input = "https://test.example.com";
        let hash = calc_hash(input, 6);

        // 複数回実行しても同じ結果であることを確認
        for _ in 0..10 {
            assert_eq!(hash, calc_hash(input, 6));
        }
    }
}
