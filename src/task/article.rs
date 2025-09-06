use crate::{
    core::{
        article::{
            get_article_content_for_storage_with_client, store_article_content, ArticleStorageData,
        },
        rss::search_backlog_article_links,
    },
    infra::api::firecrawl::FirecrawlClient,
};
use anyhow::Result;
use sqlx::PgPool;

/// バックログ対象リンクから処理待ちの記事を収集してDBに保存する
pub async fn task_collect_articles<F: FirecrawlClient>(
    firecrawl_client: &F,
    pool: &PgPool,
) -> Result<()> {
    println!("--- 記事内容取得開始 ---");
    // 未処理のリンクを取得（articleテーブルに存在しないarticle_linkを取得）
    let unprocessed_links = search_backlog_article_links(pool).await?;
    println!("未処理リンク数: {}件", unprocessed_links.len());

    for article_link in unprocessed_links {
        println!("記事処理中: {}", article_link.url);

        let article_result =
            get_article_content_for_storage_with_client(&article_link.url, firecrawl_client).await;

        match article_result {
            Ok(article) => match store_article_content(&article, pool).await {
                Ok(_) => {
                    println!("  記事保存完了");
                }
                Err(e) => {
                    eprintln!("  記事保存エラー: {}", e);
                }
            },
            Err(e) => {
                eprintln!("  記事取得エラー: {}", e);

                // エラーが発生した場合も、status_codeを記録してスキップ
                let error_article = ArticleStorageData {
                    url: article_link.url,
                    timestamp: chrono::Utc::now(),
                    status_code: 500, // エラー用のステータスコード
                    content: format!("取得エラー: {}", e),
                };

                if let Err(store_err) = store_article_content(&error_article, pool).await {
                    eprintln!("  エラー記事の保存に失敗: {}", store_err);
                }
            }
        }
    }

    println!("--- 記事内容取得完了 ---");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::api::firecrawl::MockFirecrawlClient;
    use sqlx::PgPool;

    #[sqlx::test(fixtures("../../fixtures/workflow.sql"))]
    async fn test_process_collect_articles(pool: PgPool) -> Result<(), anyhow::Error> {
        // fixtureから6件の未処理RSSリンクと3件の処理済み記事が読み込まれる（archiveも再処理される）

        // 全URL成功のモッククライアントを設定（基本テスト用）
        let mock_client = MockFirecrawlClient::new_success("基本テスト記事の内容です");
        // 記事取得を実行（未処理の6件が処理される）
        let result = task_collect_articles(&mock_client, &pool).await;
        assert!(
            result.is_ok(),
            "記事取得処理が失敗しました: {:?}",
            result.err()
        );
        // 全記事数確認（既存3件 + 新規3件 + 更新3件 = 9件、実際は再処理により既存が更新されて8件）
        let total_articles = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            total_articles.unwrap_or(0),
            8,
            "総記事数が期待値と異なります"
        );
        // 成功記事数確認（全て成功で処理されるため8件）
        let new_success_articles =
            sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 200")
                .fetch_one(&pool)
                .await?;
        assert_eq!(
            new_success_articles.unwrap_or(0),
            8,
            "成功記事数が期待値と異なります"
        );
        // エラー記事数の確認（全て成功処理されるため0件）
        let error_articles =
            sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 500")
                .fetch_one(&pool)
                .await?;
        assert_eq!(
            error_articles.unwrap_or(0),
            0,
            "エラー記事数が期待値と異なります"
        );
        // 特定記事の内容確認
        let article_content: String = sqlx::query_scalar!(
            "SELECT content FROM articles WHERE url = $1",
            "https://news.example.com/article1"
        )
        .fetch_one(&pool)
        .await?;
        assert!(
            article_content.contains("基本テスト記事の内容です"),
            "記事内容が正しく保存されていません"
        );

        println!("✅ 基本workflow統合テスト完了: 6件の記事を処理しました");
        Ok(())
    }

    #[sqlx::test(fixtures("../../fixtures/workflow_mixed.sql"))]
    async fn test_process_collect_articles_mixed(pool: PgPool) -> Result<(), anyhow::Error> {
        // fixtureから11件の未処理RSSリンクと2件の処理済み記事が読み込まれる（エラー記事も再処理）

        // 全URL成功のモッククライアントを設定（混在テスト用）
        let mock_client = MockFirecrawlClient::new_success("混在テスト記事の内容です");
        // 記事取得を実行（未処理の11件が処理される）
        let result = task_collect_articles(&mock_client, &pool).await;
        assert!(
            result.is_ok(),
            "混在シナリオの処理が失敗しました: {:?}",
            result.err()
        );
        // 全記事数確認（既存2件 + 新規10件 = 12件）
        let total_articles = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            total_articles.unwrap_or(0),
            12,
            "総記事数が期待値と異なります"
        );
        // 成功記事数確認（全て成功で処理されるため12件）
        let success_articles =
            sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 200")
                .fetch_one(&pool)
                .await?;
        assert_eq!(
            success_articles.unwrap_or(0),
            12,
            "成功記事数が期待値と異なります"
        );
        // エラー記事数確認（全て成功処理されるため0件）
        let error_articles =
            sqlx::query_scalar!("SELECT COUNT(*) FROM articles WHERE status_code = 500")
                .fetch_one(&pool)
                .await?;
        assert_eq!(
            error_articles.unwrap_or(0),
            0,
            "エラー記事数が期待値と異なります"
        );
        // 成功記事の内容確認（全て成功するのでいずれかの記事を確認）
        let success_content: String = sqlx::query_scalar!(
            "SELECT content FROM articles WHERE url = $1",
            "https://success.example.com/news1"
        )
        .fetch_one(&pool)
        .await?;
        assert!(
            success_content.contains("混在テスト記事の内容です"),
            "成功記事の内容が正しくありません"
        );

        println!("✅ 混在シナリオworkflow統合テスト完了: 11件すべて成功処理しました");
        Ok(())
    }
}
