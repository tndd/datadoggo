use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::core::link::model::ArticleLink;

/// # 概要
/// ArticleLinkの配列を指定されたデータベースプールに保存する。
///
/// # Note
/// sqlxの推奨パターンに従い、sqlx::query!マクロを使用してコンパイル時安全性を確保しています。
pub async fn store_article_links(article_links: &[ArticleLink], pool: &PgPool) -> Result<()> {
    if article_links.is_empty() {
        return Ok(());
    }

    // 配列として渡すためのデータ準備
    let urls: Vec<String> = article_links.iter().map(|r| r.url.clone()).collect();
    let titles: Vec<String> = article_links.iter().map(|r| r.title.clone()).collect();
    let pub_dates: Vec<DateTime<Utc>> = article_links.iter().map(|r| r.pub_date).collect();
    let sources: Vec<String> = article_links.iter().map(|r| r.source.clone()).collect();

    // バルクUPSERT処理
    sqlx::query!(
        r#"
        INSERT INTO article_links (url, title, pub_date, source)
        SELECT * FROM UNNEST($1::text[], $2::text[], $3::timestamptz[], $4::text[])
        ON CONFLICT (url) DO UPDATE SET
            title = EXCLUDED.title,
            pub_date = EXCLUDED.pub_date,
            source = EXCLUDED.source
        WHERE (article_links.title, article_links.pub_date, article_links.source)
            IS DISTINCT FROM (EXCLUDED.title, EXCLUDED.pub_date, EXCLUDED.source)
        "#,
        &urls,
        &titles,
        &pub_dates,
        &sources
    )
    .execute(pool)
    .await
    .context("記事リンクのバルクUPSERT処理に失敗しました")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::storage::db::setup_test_db;

    // データベース保存機能のテスト（関数名ベースに統一）
    mod store_article_links {
        use super::*;

        #[tokio::test]
        async fn test_save_links_to_db() -> Result<(), anyhow::Error> {
            let (_container, pool) = setup_test_db().await;
            // テスト用リンクデータを作成（必須フィールドのみ）
            let rss_basic = vec![
                ArticleLink {
                    title: "Test Article 1".to_string(),
                    url: "https://test.example.com/article1".to_string(),
                    pub_date: "2025-08-26T10:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
                ArticleLink {
                    title: "Test Article 2".to_string(),
                    url: "https://test.example.com/article2".to_string(),
                    pub_date: "2025-08-26T11:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
                ArticleLink {
                    title: "異なるドメイン記事".to_string(),
                    url: "https://different.domain.com/post".to_string(),
                    pub_date: "2025-08-26T12:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
            ];

            // データベースに保存をテスト
            store_article_links(&rss_basic, &pool).await?;

            // 実際にデータベースに保存されたことを確認
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, Some(3), "期待する件数(3件)が保存されませんでした");

            println!("✅ RSSリンク保存テスト成功: 3件");

            Ok(())
        }

        #[sqlx::test(fixtures("command"))]
        async fn test_duplicate_links(pool: PgPool) -> Result<(), anyhow::Error> {
            // fixtureで既に17件のデータが存在している状態

            // 同じリンクの記事を作成（重複）
            let duplicate_article_link = ArticleLink {
                title: "異なるタイトル".to_string(),
                url: "https://test.example.com/article1".to_string(), // fixtureと同じリンク
                pub_date: "2025-08-26T13:00:00Z".parse().unwrap(),
                source: "test".to_string(),
            };

            // 重複記事を保存しようとする
            store_article_links(&[duplicate_article_link], &pool).await?;

            // データベースの件数は変わらない（19件のまま）
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                count,
                Some(17),
                "重複記事が挿入され、件数が変わってしまいました"
            );

            println!("✅ RSS重複スキップ検証成功");

            Ok(())
        }

        #[sqlx::test(fixtures("command"))]
        async fn test_mixed_new_and_existing_links(pool: PgPool) -> Result<(), anyhow::Error> {
            // fixtureで既に17件のデータが存在している状態

            // 1件は既存（重複）、2件は新規のデータを作成
            let mixed_articles = vec![
                ArticleLink {
                    title: "既存記事".to_string(),
                    url: "https://test.example.com/article1".to_string(), // fixtureと同じリンク
                    pub_date: "2025-08-26T14:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
                ArticleLink {
                    title: "新規記事1".to_string(),
                    url: "https://test.example.com/new-article1".to_string(), // 新しいリンク
                    pub_date: "2025-08-26T15:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
                ArticleLink {
                    title: "新規記事2".to_string(),
                    url: "https://another.domain.com/article".to_string(), // 異なるドメイン
                    pub_date: "2025-08-26T16:00:00Z".parse().unwrap(),
                    source: "test".to_string(),
                },
            ];

            store_article_links(&mixed_articles, &pool).await?;

            // 最終的にデータベースには19件（fixture 17件 + 新規 2件）
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, Some(19), "期待する件数(19件)と異なります");

            println!("✅ RSS混在データ処理検証成功");

            Ok(())
        }
    }

    // store_article_links のエッジケース
    mod store_article_links_edge_cases {
        use super::*;

        #[sqlx::test(fixtures("command_error_cases"))]
        async fn test_store_article_links_with_special_characters(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // 既存のfixture データを確認
            let initial_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;

            // 特殊文字を含む新しい記事リンクを追加
            let special_links = vec![
                ArticleLink {
                    title: "SQL インジェクションテスト'; DROP TABLE articles; --".to_string(),
                    url: "https://security-test.example.com/sql-injection-attempt".to_string(),
                    pub_date: "2025-01-21T10:00:00Z".parse().unwrap(),
                    source: "security-test".to_string(),
                },
                ArticleLink {
                    title: "XSS テスト <script>alert('xss')</script>".to_string(),
                    url: "https://security-test.example.com/xss-attempt".to_string(),
                    pub_date: "2025-01-21T11:00:00Z".parse().unwrap(),
                    source: "security-test".to_string(),
                },
            ];

            // 特殊文字を含む記事を保存
            let result = store_article_links(&special_links, &pool).await;
            assert!(
                result.is_ok(),
                "特殊文字を含む記事の保存が失敗しました: {:?}",
                result.err()
            );

            // 保存後の件数確認
            let final_count = sqlx::query_scalar!("SELECT COUNT(*) FROM article_links")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                final_count.unwrap_or(0),
                initial_count.unwrap_or(0) + 2,
                "特殊文字記事が正しく保存されませんでした"
            );

            // 実際に保存されたデータの確認
            let saved_article: Option<String> = sqlx::query_scalar!(
                "SELECT title FROM article_links WHERE url = $1",
                "https://security-test.example.com/sql-injection-attempt"
            )
            .fetch_optional(&pool)
            .await?;

            assert!(
                saved_article.is_some(),
                "SQL インジェクション対策記事が見つかりません"
            );
            assert!(
                saved_article
                    .unwrap()
                    .contains("'; DROP TABLE articles; --"),
                "特殊文字が正しくエスケープされて保存されていません"
            );

            println!("✅ 特殊文字・セキュリティテスト完了");
            Ok(())
        }

        #[sqlx::test(fixtures("command_error_cases"))]
        async fn test_upsert_with_different_source_values(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            // 同じURL、異なるsourceでの記事作成
            let duplicate_url = "https://duplicate-source.example.com/same-article";
            // Vecではなく固定長配列で十分なため、Clippyに従い配列に変更
            let links_with_different_sources = [
                ArticleLink {
                    title: "初回の記事".to_string(),
                    url: duplicate_url.to_string(),
                    pub_date: "2025-01-21T12:00:00Z".parse().unwrap(),
                    source: "first-source".to_string(),
                },
                ArticleLink {
                    title: "更新された記事".to_string(),
                    url: duplicate_url.to_string(),
                    pub_date: "2025-01-21T13:00:00Z".parse().unwrap(),
                    source: "second-source".to_string(),
                },
            ];

            // 1回目：初回保存
            store_article_links(&links_with_different_sources[0..1], &pool).await?;

            let first_save: (String, String) = sqlx::query_as!(
                ArticleLink,
                "SELECT url, title, pub_date, source FROM article_links WHERE url = $1",
                duplicate_url
            )
            .fetch_one(&pool)
            .await
            .map(|link| (link.title, link.source))?;

            assert_eq!(first_save.0, "初回の記事");
            assert_eq!(first_save.1, "first-source");

            // 2回目：更新（UPSERT）
            store_article_links(&links_with_different_sources[1..2], &pool).await?;

            let updated_save: (String, String) = sqlx::query_as!(
                ArticleLink,
                "SELECT url, title, pub_date, source FROM article_links WHERE url = $1",
                duplicate_url
            )
            .fetch_one(&pool)
            .await
            .map(|link| (link.title, link.source))?;

            assert_eq!(updated_save.0, "更新された記事");
            assert_eq!(updated_save.1, "second-source");

            // 同じURLの記事が1件のみ存在することを確認
            let count: i64 = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM article_links WHERE url = $1",
                duplicate_url
            )
            .fetch_one(&pool)
            .await?
            .unwrap_or(0);

            assert_eq!(count, 1, "同じURLの記事は1件のみであるべきです");

            println!("✅ 異なるsource値でのUPSERTテスト完了");
            Ok(())
        }
    }
}
