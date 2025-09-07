use crate::infra::api::firecrawl::{FirecrawlClient, ReqwestFirecrawlClient};
use anyhow::{Context, Result};
use sqlx::PgPool;

use super::repository;
use super::types::{
    ArticleContent, ArticleContentQuery, ArticleJoinRow, ArticleJoinRowQuery, ArticleUrlStatus,
    ArticleUrlStatusQuery,
};

// ==============================
// ビジネスロジック層の公開API
// ==============================

/// ArticleUrlStatusを取得する（リポジトリ層への委譲）
pub async fn search_article_url_statuses(
    query: Option<ArticleUrlStatusQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleUrlStatus>> {
    repository::search_article_url_statuses(query, pool).await
}

/// ArticleJoinRowを取得する（リポジトリ層への委譲）
pub async fn search_article_join_rows(
    query: Option<ArticleJoinRowQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleJoinRow>> {
    repository::search_article_join_rows(query, pool).await
}

/// ArticleContentを取得する（リポジトリ層への委譲）
pub async fn search_article_contents(
    query: Option<ArticleContentQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleContent>> {
    repository::search_article_contents(query, pool).await
}

// ==============================
// 外部API連携とビジネスロジック
// ==============================

/// URLから記事内容を取得してArticleContent構造体に変換する（Firecrawl SDK使用）
pub async fn get_article_content(url: &str) -> Result<ArticleContent> {
    let client = ReqwestFirecrawlClient::new().context("記事取得クライアントの初期化に失敗")?;
    get_article_content_with_client(url, &client).await
}

/// 指定されたFirecrawlクライアントを使用して記事内容を取得
///
/// この関数は依存注入をサポートし、テスト時にモッククライアントを
/// 注入することでFirecrawl APIへの実際の通信を避けることができます。
pub async fn get_article_content_with_client(
    url: &str,
    client: &dyn FirecrawlClient,
) -> Result<ArticleContent> {
    match client.scrape_url(url).await {
        Ok(result) => Ok(ArticleContent {
            url: url.to_string(),
            timestamp: chrono::Utc::now(),
            status_code: 200,
            content: result
                .markdown
                .unwrap_or_else(|| "記事内容が取得できませんでした".to_string()),
        }),
        Err(e) => Ok(ArticleContent {
            url: url.to_string(),
            timestamp: chrono::Utc::now(),
            status_code: 500,
            content: format!("記事取得APIエラー: {}", e),
        }),
    }
}

/// URLから記事を取得してデータベースに保存する統合関数
pub async fn fetch_and_store_article(url: &str, pool: &PgPool) -> Result<ArticleContent> {
    let article = get_article_content(url).await?;
    repository::store_article_content(&article, pool).await?;
    Ok(article)
}

/// 指定されたクライアントを使って記事を取得してデータベースに保存する統合関数（テスト用）
pub async fn fetch_and_store_article_with_client(
    url: &str,
    client: &dyn FirecrawlClient,
    pool: &PgPool,
) -> Result<ArticleContent> {
    let article = get_article_content_with_client(url, client).await?;
    repository::store_article_content(&article, pool).await?;
    Ok(article)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::storage::file::load_json_from_file;
    use chrono::Utc;

    // tests直下のヘルパー関数
    fn read_article_content_from_file(file_path: &str) -> Result<ArticleContent> {
        let json_value = load_json_from_file(file_path)?;
        let content = json_value
            .get("markdown")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("markdownフィールドが見つかりません"))?
            .to_string();
        let metadata = json_value
            .get("metadata")
            .ok_or_else(|| anyhow::anyhow!("metadataフィールドが見つかりません"))?;
        let url = metadata
            .get("url")
            .and_then(|v| v.as_str())
            .or_else(|| metadata.get("sourceURL").and_then(|v| v.as_str()))
            .ok_or_else(|| anyhow::anyhow!("URLが見つかりません"))?
            .to_string();
        let status_code = metadata
            .get("statusCode")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .ok_or_else(|| anyhow::anyhow!("statusCodeフィールドが見つかりません"))?;
        let now = Utc::now();

        Ok(ArticleContent {
            url,
            timestamp: now,
            status_code,
            content,
        })
    }

    mod helper {
        use super::*;

        #[test]
        fn test_read_article_content_from_file() {
            let result = read_article_content_from_file("mock/fc/bbc.json");
            assert!(result.is_ok());
            let article = result.unwrap();
            assert!(!article.url.is_empty());
            assert_eq!(article.status_code, 200);
        }
    }

    // オンラインテストはモジュール単位でfeatureガード
    #[cfg(feature = "online")]
    mod online {
        use super::*;

        #[tokio::test]
        async fn test_get_article_content() {
            let result = get_article_content("https://httpbin.org/html").await;
            assert!(result.is_ok());
        }
    }

    use super::super::types::ArticleStatus;

    #[test]
    fn test_article_status_enum() {
        let unprocessed = ArticleStatus::Unprocessed;
        let success = ArticleStatus::Success;
        let error_404 = ArticleStatus::Error(404);

        assert!(matches!(unprocessed, ArticleStatus::Unprocessed));
        assert!(matches!(success, ArticleStatus::Success));
        assert!(matches!(error_404, ArticleStatus::Error(404)));
    }

    #[test]
    fn test_article_content_from_file() {
        let article_result = read_article_content_from_file("mock/fc/bbc.json");
        assert!(article_result.is_ok());
        let article = article_result.unwrap();
        assert!(!article.content.is_empty());
        assert!(article.content.contains("Gaza"));
    }

    // 上記onlineモジュールに集約済み
}
