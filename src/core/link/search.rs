use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::core::link::model::{ArticleLink, ArticleLinkQuery};

/// # 概要
/// 指定されたデータベースプールから記事リンクを取得する。
pub async fn search_article_links(
    query: Option<ArticleLinkQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleLink>> {
    let query = query.unwrap_or_default();

    // 単一の静的SQL + オプション引数方式
    let article_links = sqlx::query_as!(
        ArticleLink,
        r#"
        SELECT url, title, pub_date, source
        FROM article_links
        WHERE
            ($1::text IS NULL OR url ILIKE '%' || $1 || '%')
            AND ($2::timestamptz IS NULL OR pub_date >= $2)
            AND ($3::timestamptz IS NULL OR pub_date <= $3)
        ORDER BY pub_date DESC
        "#,
        query.link_pattern,
        query.pub_date_from,
        query.pub_date_to
    )
    .fetch_all(pool)
    .await?;

    Ok(article_links)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::parser::parse_date;

    // 日付ソートの検証ヘルパー関数
    fn validate_date_sort_desc(article_links: &[ArticleLink]) {
        let mut prev_date: Option<DateTime<Utc>> = None;
        for article_link in article_links {
            if let Some(prev) = prev_date {
                assert!(
                    article_link.pub_date <= prev,
                    "日付の降順ソートが正しくありません"
                );
            }
            prev_date = Some(article_link.pub_date);
        }
    }

    // データベース取得機能のテスト（関数名ベースに統一）
    mod search_article_links {
        use super::*;

        #[sqlx::test(fixtures("search"))]
        async fn test_search_all_article_links_comprehensive(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // 統合フィクスチャで19件のデータが存在

            let article_links = search_article_links(None, &pool).await?;

            // 全件取得されることを確認
            assert!(
                article_links.len() >= 17,
                "全件取得で最低17件が期待されます"
            );

            // 基本的な検証（ソート順、フィールド存在）
            validate_date_sort_desc(&article_links);

            println!("✅ RSS全件取得際どいテスト成功: {}件", article_links.len());

            Ok(())
        }

        #[sqlx::test(fixtures("search"))]
        async fn test_date_filtering_comprehensive(pool: PgPool) -> Result<(), anyhow::Error> {
            // 開始境界時刻の記事テスト
            let filter_start_boundary = ArticleLinkQuery {
                link_pattern: None,
                pub_date_from: Some(parse_date("2025-01-15T00:00:00Z")?),
                pub_date_to: Some(parse_date("2025-01-15T00:00:01Z")?),
            };
            let article_links_start =
                search_article_links(Some(filter_start_boundary), &pool).await?;
            assert_eq!(article_links_start.len(), 1);
            assert_eq!(
                article_links_start[0].url,
                "https://test.com/boundary/exactly-start"
            );

            // 終了境界時刻の記事テスト
            let filter_end_boundary = ArticleLinkQuery {
                link_pattern: None,
                pub_date_from: Some(parse_date("2025-01-15T23:59:58Z")?),
                pub_date_to: Some(parse_date("2025-01-15T23:59:59Z")?),
            };
            let article_links_end = search_article_links(Some(filter_end_boundary), &pool).await?;
            assert_eq!(article_links_end.len(), 1);
            assert_eq!(
                article_links_end[0].url,
                "https://test.com/boundary/exactly-end"
            );

            // 1日全体の境界記事確認
            let filter_full_day = ArticleLinkQuery {
                link_pattern: None,
                pub_date_from: Some(parse_date("2025-01-15T00:00:00Z")?),
                pub_date_to: Some(parse_date("2025-01-15T23:59:59Z")?),
            };
            let article_links_day = search_article_links(Some(filter_full_day), &pool).await?;
            let day_links: Vec<&str> = article_links_day.iter().map(|a| a.url.as_str()).collect();
            assert!(day_links.contains(&"https://test.com/boundary/exactly-start"));
            assert!(day_links.contains(&"https://test.com/boundary/exactly-end"));
            assert!(day_links.contains(&"https://example.com/tech/article-2025-01-15"));
            assert!(!day_links.contains(&"https://test.com/boundary/one-second-before"));
            assert!(!day_links.contains(&"https://test.com/boundary/one-second-after"));

            println!("✅ RSS日付境界総合テスト成功");
            Ok(())
        }

        // ここに edge_cases 由来の3テストを統合
        #[sqlx::test(fixtures("search_edge_cases"))]
        async fn test_search_article_links_boundary_conditions(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // 空文字タイトルの検索
            let empty_title_results = search_article_links(
                Some(ArticleLinkQuery {
                    link_pattern: Some("empty-title".to_string()),
                    pub_date_from: None,
                    pub_date_to: None,
                }),
                &pool,
            )
            .await?;

            assert_eq!(
                empty_title_results.len(),
                1,
                "空文字タイトル記事が見つかりませんでした"
            );
            assert_eq!(empty_title_results[0].title, "");

            // 非常に短いURL
            let short_results = search_article_links(
                Some(ArticleLinkQuery {
                    link_pattern: Some("x.co".to_string()),
                    pub_date_from: None,
                    pub_date_to: None,
                }),
                &pool,
            )
            .await?;
            assert_eq!(short_results.len(), 1, "最短URL記事が見つかりませんでした");
            assert_eq!(short_results[0].title, "最短URL記事");

            // 長いURL
            let long_url_results = search_article_links(
                Some(ArticleLinkQuery {
                    link_pattern: Some("very-long-subdomain".to_string()),
                    pub_date_from: None,
                    pub_date_to: None,
                }),
                &pool,
            )
            .await?;
            assert_eq!(
                long_url_results.len(),
                1,
                "長いURL記事が見つかりませんでした"
            );
            assert_eq!(long_url_results[0].title, "長いURL記事");

            Ok(())
        }

        #[sqlx::test(fixtures("search_edge_cases"))]
        async fn test_date_precision_and_boundaries(pool: PgPool) -> Result<(), anyhow::Error> {
            // マイクロ秒精度の日付検索
            let precise_date_results = search_article_links(
                Some(ArticleLinkQuery {
                    link_pattern: Some("microsecond".to_string()),
                    pub_date_from: None,
                    pub_date_to: None,
                }),
                &pool,
            )
            .await?;
            assert_eq!(
                precise_date_results.len(),
                1,
                "マイクロ秒精度記事が見つかりませんでした"
            );

            // UNIX エポック境界
            let epoch_results = search_article_links(
                Some(ArticleLinkQuery {
                    link_pattern: None,
                    pub_date_from: Some(parse_date("1970-01-01T00:00:00Z")?),
                    pub_date_to: Some(parse_date("1970-01-01T00:00:02Z")?),
                }),
                &pool,
            )
            .await?;
            assert_eq!(
                epoch_results.len(),
                1,
                "UNIX エポック記事が見つかりませんでした"
            );
            assert_eq!(epoch_results[0].title, "UNIX開始日記事");

            // うるう年境界（2024-02-29）
            let leap_results = search_article_links(
                Some(ArticleLinkQuery {
                    link_pattern: None,
                    pub_date_from: Some(parse_date("2024-02-29T00:00:00Z")?),
                    pub_date_to: Some(parse_date("2024-02-29T23:59:59Z")?),
                }),
                &pool,
            )
            .await?;
            assert_eq!(leap_results.len(), 1, "うるう年記事が見つかりませんでした");
            assert_eq!(leap_results[0].title, "うるう年記事");
            Ok(())
        }

        #[sqlx::test(fixtures("search_edge_cases"))]
        async fn test_case_insensitive_search(pool: PgPool) -> Result<(), anyhow::Error> {
            // 大文字小文字混在URLの検索（ILIKE動作確認）
            let case_results_lower = search_article_links(
                Some(ArticleLinkQuery {
                    link_pattern: Some("casesensitive".to_string()), // 小文字で検索
                    pub_date_from: None,
                    pub_date_to: None,
                }),
                &pool,
            )
            .await?;
            assert!(
                case_results_lower.len() >= 2,
                "大文字小文字を含むURL検索が正しく動作していません: {}件",
                case_results_lower.len()
            );
            let urls: Vec<&str> = case_results_lower
                .iter()
                .map(|link| link.url.as_str())
                .collect();
            assert!(urls.iter().any(|url| url.contains("CaseSensitive")));
            assert!(urls.iter().any(|url| url.contains("casesensitive")));
            Ok(())
        }
    }
}
