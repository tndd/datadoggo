use crate::infra::api::firecrawl::{FirecrawlClient, ReqwestFirecrawlClient};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

// クエリビルダーヘルパー構造体
struct QueryBuilder {
    query_builder: sqlx::QueryBuilder<'static, sqlx::Postgres>,
    has_where: bool,
}

impl QueryBuilder {
    fn new(base_query: &'static str) -> Self {
        Self {
            query_builder: sqlx::QueryBuilder::<sqlx::Postgres>::new(base_query),
            has_where: false,
        }
    }

    // WHERE句または AND句を適切に追加
    fn add_condition(&mut self) -> &mut sqlx::QueryBuilder<'static, sqlx::Postgres> {
        if self.has_where {
            self.query_builder.push(" AND ");
        } else {
            self.query_builder.push(" WHERE ");
            self.has_where = true;
        }
        &mut self.query_builder
    }

    // OR条件グループを開始
    fn start_or_group(&mut self) -> &mut sqlx::QueryBuilder<'static, sqlx::Postgres> {
        if self.has_where {
            self.query_builder.push(" AND (");
        } else {
            self.query_builder.push(" WHERE (");
            self.has_where = true;
        }
        &mut self.query_builder
    }

    // URLパターンマッチング条件を追加
    fn add_url_pattern_condition(&mut self, table_alias: &str, url_pattern: &str) {
        let pattern = format!("%{}%", url_pattern);
        self.add_condition()
            .push(format!("{}.url ILIKE ", table_alias))
            .push_bind(pattern);
    }

    // ステータス条件を追加
    fn add_status_conditions(&mut self, statuses: &[ArticleStatus]) {
        self.start_or_group();
        for (i, status) in statuses.iter().enumerate() {
            if i > 0 {
                self.query_builder.push(" OR ");
            }
            match status {
                ArticleStatus::Unprocessed => {
                    self.query_builder.push("a.status_code IS NULL");
                }
                ArticleStatus::Success => {
                    self.query_builder.push("a.status_code = 200");
                }
                ArticleStatus::Error(code) => {
                    self.query_builder.push("a.status_code = ").push_bind(*code);
                }
            }
        }
        self.query_builder.push(")");
    }

    // 日付範囲条件を追加
    fn add_date_range_condition(
        &mut self,
        field: &str,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    ) {
        if let Some(date_from) = from {
            self.add_condition()
                .push(format!("{} >= ", field))
                .push_bind(date_from);
        }
        if let Some(date_to) = to {
            self.add_condition()
                .push(format!("{} <= ", field))
                .push_bind(date_to);
        }
    }

    // ソース条件を追加
    fn add_source_condition(&mut self, source: String) {
        self.add_condition().push("al.source = ").push_bind(source);
    }

    // ステータスコード条件を追加
    fn add_status_code_condition(&mut self, status_code: i32) {
        self.add_condition()
            .push("status_code = ")
            .push_bind(status_code);
    }

    // ORDER BY句を追加
    fn add_order_by(&mut self, order_clause: &str) {
        self.query_builder.push(" ORDER BY ").push(order_clause);
    }

    // LIMIT句を追加
    fn add_limit(&mut self, limit: i64) {
        self.query_builder.push(" LIMIT ").push_bind(limit);
    }

    // QueryBuilderを取得
    fn build(self) -> sqlx::QueryBuilder<'static, sqlx::Postgres> {
        self.query_builder
    }
}

// 記事の処理状態を表現するenum（model.rsから移動）
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

// ArticleUrlStatusを取得する関数
pub async fn search_article_url_statuses(
    query: Option<ArticleUrlStatusQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleUrlStatus>> {
    let query = query.unwrap_or_default();

    let mut qb = QueryBuilder::new(
        r#"
        SELECT 
            al.url,
            a.status_code
        FROM article_links al
        LEFT JOIN articles a ON al.url = a.url
        "#,
    );

    // URL パターン条件
    if let Some(ref url_pattern) = query.url_pattern {
        qb.add_url_pattern_condition("al", url_pattern);
    }

    // ステータス条件
    if let Some(ref statuses) = query.statuses {
        qb.add_status_conditions(statuses);
    }

    // ソート条件
    qb.add_order_by("al.url");

    // LIMIT条件
    if let Some(limit) = query.limit {
        qb.add_limit(limit);
    }

    let results = qb
        .build()
        .build_query_as::<ArticleUrlStatus>()
        .fetch_all(pool)
        .await
        .context("記事URL状態情報の取得に失敗")?;

    Ok(results)
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

// ArticleJoinRowQueryを受け取り、Vec<ArticleJoinRow>を返す関数
pub async fn search_article_join_rows(
    query: Option<ArticleJoinRowQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleJoinRow>> {
    let query = query.unwrap_or_default();

    let mut qb = QueryBuilder::new(
        r#"
        SELECT 
            al.url,
            al.title,
            al.pub_date,
            al.source,
            a.timestamp,
            a.status_code,
            a.content
        FROM article_links al
        LEFT JOIN articles a ON al.url = a.url
        "#,
    );

    // URL パターン条件
    if let Some(ref link_pattern) = query.link_pattern {
        qb.add_url_pattern_condition("al", link_pattern);
    }

    // 日付範囲条件
    qb.add_date_range_condition("al.pub_date", query.pub_date_from, query.pub_date_to);

    // ステータス条件
    if let Some(ref statuses) = query.statuses {
        qb.add_status_conditions(statuses);
    }

    // ソース条件
    if let Some(source) = query.source {
        qb.add_source_condition(source);
    }

    // ソート条件
    qb.add_order_by("al.pub_date DESC");

    // LIMIT条件
    if let Some(limit) = query.limit {
        qb.add_limit(limit);
    }

    let results = qb
        .build()
        .build_query_as::<ArticleJoinRow>()
        .fetch_all(pool)
        .await
        .context("記事結合情報の取得に失敗")?;

    Ok(results)
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleContent {
    pub url: String,
    pub timestamp: DateTime<Utc>, // (updated_at)
    pub status_code: i32,
    pub content: String,
}

#[derive(Debug, Default)]
pub struct ArticleContentQuery {
    pub url_pattern: Option<String>,
    pub timestamp_from: Option<DateTime<Utc>>,
    pub timestamp_to: Option<DateTime<Utc>>,
    pub status_code: Option<i32>,
}

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

/// 記事内容をデータベースに保存する。
/// 重複した場合には更新を行う。
pub async fn store_article_content(article: &ArticleContent, pool: &PgPool) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO articles (url, status_code, content)
        VALUES ($1, $2, $3)
        ON CONFLICT (url) DO UPDATE SET 
            status_code = EXCLUDED.status_code,
            content = EXCLUDED.content,
            timestamp = CURRENT_TIMESTAMP
        WHERE (articles.status_code, articles.content)
            IS DISTINCT FROM (EXCLUDED.status_code, EXCLUDED.content)
        "#,
        article.url,
        article.status_code,
        article.content
    )
    .execute(pool)
    .await
    .context("記事データのデータベース保存に失敗")?;

    Ok(())
}

/// URLから記事を取得してデータベースに保存する統合関数
pub async fn fetch_and_store_article(url: &str, pool: &PgPool) -> Result<ArticleContent> {
    let article = get_article_content(url).await?;
    store_article_content(&article, pool).await?;
    Ok(article)
}

/// 指定されたクライアントを使って記事を取得してデータベースに保存する統合関数（テスト用）
pub async fn fetch_and_store_article_with_client(
    url: &str,
    client: &dyn FirecrawlClient,
    pool: &PgPool,
) -> Result<ArticleContent> {
    let article = get_article_content_with_client(url, client).await?;
    store_article_content(&article, pool).await?;
    Ok(article)
}

/// 指定されたデータベースプールからArticleContentを取得する。
pub async fn search_article_contents(
    query: Option<ArticleContentQuery>,
    pool: &PgPool,
) -> Result<Vec<ArticleContent>> {
    let query = query.unwrap_or_default();

    let mut qb = QueryBuilder::new("SELECT url, timestamp, status_code, content FROM articles");

    // URL パターン条件
    if let Some(ref url_pattern) = query.url_pattern {
        // articles テーブルなのでテーブル名なし
        let pattern = format!("%{}%", url_pattern);
        qb.add_condition().push("url ILIKE ").push_bind(pattern);
    }

    // 日付範囲条件
    qb.add_date_range_condition("timestamp", query.timestamp_from, query.timestamp_to);

    // ステータスコード条件
    if let Some(status) = query.status_code {
        qb.add_status_code_condition(status);
    }

    // ソート条件
    qb.add_order_by("timestamp DESC");

    let articles = qb
        .build()
        .build_query_as::<ArticleContent>()
        .fetch_all(pool)
        .await?;

    Ok(articles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::storage::file::load_json_from_file;

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

    #[test]
    fn test_article_status_enum() {
        // 各バリアントの基本動作テスト
        let unprocessed = ArticleStatus::Unprocessed;
        let success = ArticleStatus::Success;
        let error_404 = ArticleStatus::Error(404);
        let error_500 = ArticleStatus::Error(500);

        // パターンマッチングの動作確認
        match unprocessed {
            ArticleStatus::Unprocessed => (),
            _ => panic!("Unprocessedのマッチングが失敗"),
        }

        match success {
            ArticleStatus::Success => (),
            _ => panic!("Successのマッチングが失敗"),
        }

        match error_404 {
            ArticleStatus::Error(404) => (),
            _ => panic!("Error(404)のマッチングが失敗"),
        }

        // シリアライゼーション/デシリアライゼーションテスト
        let json_success = serde_json::to_string(&success).unwrap();
        let deserialized: ArticleStatus = serde_json::from_str(&json_success).unwrap();
        match deserialized {
            ArticleStatus::Success => (),
            _ => panic!("シリアライゼーション後のデシリアライズが失敗"),
        }

        // エラーコードの値検証
        match error_500 {
            ArticleStatus::Error(code) => assert_eq!(code, 500),
            _ => panic!("Error(500)の値取得が失敗"),
        }
    }

    #[test]
    fn test_query_structures() {
        use chrono::{TimeZone, Utc};

        // ArticleUrlStatusQueryのデフォルト値テスト
        let default_url_query = ArticleUrlStatusQuery::default();
        assert!(default_url_query.url_pattern.is_none());
        assert!(default_url_query.statuses.is_none());
        assert!(default_url_query.limit.is_none());

        // ArticleJoinRowQueryのデフォルト値テスト
        let default_join_query = ArticleJoinRowQuery::default();
        assert!(default_join_query.link_pattern.is_none());
        assert!(default_join_query.pub_date_from.is_none());
        assert!(default_join_query.pub_date_to.is_none());
        assert!(default_join_query.statuses.is_none());
        assert!(default_join_query.source.is_none());
        assert!(default_join_query.limit.is_none());

        // ArticleContentQueryのデフォルト値テスト
        let default_content_query = ArticleContentQuery::default();
        assert!(default_content_query.url_pattern.is_none());
        assert!(default_content_query.timestamp_from.is_none());
        assert!(default_content_query.timestamp_to.is_none());
        assert!(default_content_query.status_code.is_none());

        // 複合条件の構築テスト
        let pub_date_from = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let pub_date_to = Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap();

        let complex_join_query = ArticleJoinRowQuery {
            link_pattern: Some("example.com".to_string()),
            pub_date_from: Some(pub_date_from),
            pub_date_to: Some(pub_date_to),
            statuses: Some(vec![ArticleStatus::Success, ArticleStatus::Error(404)]),
            source: Some("test-feed".to_string()),
            limit: Some(10),
        };

        assert_eq!(
            complex_join_query.link_pattern,
            Some("example.com".to_string())
        );
        assert_eq!(complex_join_query.pub_date_from, Some(pub_date_from));
        assert_eq!(complex_join_query.pub_date_to, Some(pub_date_to));
        assert_eq!(complex_join_query.statuses.as_ref().unwrap().len(), 2);
        assert_eq!(complex_join_query.source, Some("test-feed".to_string()));
        assert_eq!(complex_join_query.limit, Some(10));

        // 境界値パターンのテスト
        let boundary_query = ArticleContentQuery {
            url_pattern: Some("".to_string()), // 空文字パターン
            timestamp_from: Some(pub_date_from),
            timestamp_to: Some(pub_date_from), // 同じ日時での範囲
            status_code: Some(0),              // 境界値
        };

        assert_eq!(boundary_query.url_pattern, Some("".to_string()));
        assert_eq!(boundary_query.timestamp_from, boundary_query.timestamp_to);
        assert_eq!(boundary_query.status_code, Some(0));
    }

    mod helper {
        use super::*;

        #[test]
        fn test_read_article_content_from_file() {
            use std::fs;

            // 正常なファイル読み込みテスト
            let result = read_article_content_from_file("mock/fc/bbc.json");
            assert!(result.is_ok(), "記事データJSONファイルの読み込みに失敗");

            let article = result.unwrap();
            assert!(!article.content.is_empty(), "contentが空です");
            assert!(!article.url.is_empty(), "URLが空です");
            assert!(article.status_code > 0, "status_codeが無効です");

            // statusCode欠損エラーテスト
            let missing_status_json = r#"
            {
                "markdown": "テスト記事の内容です",
                "metadata": {
                    "url": "https://test.example.com/article"
                }
            }
            "#;
            let temp_file1 = "temp_missing_status.json";
            fs::write(temp_file1, missing_status_json).expect("テストファイルの作成に失敗");
            let result = read_article_content_from_file(temp_file1);
            assert!(
                result.is_err(),
                "statusCodeが存在しないのにエラーにならなかった"
            );
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("statusCodeフィールドが見つかりません"));
            fs::remove_file(temp_file1).ok();

            // URL欠損エラーテスト
            let missing_url_json = r#"
            {
                "markdown": "テスト記事の内容です",
                "metadata": {
                    "statusCode": 200
                }
            }
            "#;
            let temp_file2 = "temp_missing_url.json";
            fs::write(temp_file2, missing_url_json).expect("テストファイルの作成に失敗");
            let result = read_article_content_from_file(temp_file2);
            assert!(result.is_err(), "URLが存在しないのにエラーにならなかった");
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("URLが見つかりません"));
            fs::remove_file(temp_file2).ok();

            // 不正JSON形式テスト
            let invalid_json = "{ invalid json }";
            let temp_file3 = "temp_invalid.json";
            fs::write(temp_file3, invalid_json).expect("テストファイルの作成に失敗");
            let result = read_article_content_from_file(temp_file3);
            assert!(result.is_err(), "不正JSONでエラーにならなかった");
            fs::remove_file(temp_file3).ok();

            // 空ファイルテスト
            let temp_file4 = "temp_empty.json";
            fs::write(temp_file4, "").expect("空ファイルの作成に失敗");
            let result = read_article_content_from_file(temp_file4);
            assert!(result.is_err(), "空ファイルでエラーにならなかった");
            fs::remove_file(temp_file4).ok();

            // sourceURL代替パターンテスト
            let source_url_json = r#"
            {
                "markdown": "sourceURL使用の記事内容",
                "metadata": {
                    "sourceURL": "https://sourceurl.example.com/article",
                    "statusCode": 200
                }
            }
            "#;
            let temp_file5 = "temp_source_url.json";
            fs::write(temp_file5, source_url_json).expect("テストファイルの作成に失敗");
            let result = read_article_content_from_file(temp_file5);
            assert!(result.is_ok(), "sourceURL形式の読み込みが失敗");
            let article = result.unwrap();
            assert_eq!(article.url, "https://sourceurl.example.com/article");
            fs::remove_file(temp_file5).ok();
        }
    }

    mod get_article_content_with_client {
        use super::*;

        #[tokio::test]
        async fn test_successful_content_retrieval() -> Result<(), anyhow::Error> {
            use crate::infra::api::firecrawl::MockFirecrawlClient;

            let test_url = "https://test.com/article";
            let mock_content = "テスト記事内容\n\nこれはモックコンテンツです。";
            let mock_client = MockFirecrawlClient::new_success(mock_content);
            let article = get_article_content_with_client(test_url, &mock_client).await?;

            assert_eq!(article.url, test_url);
            assert_eq!(article.status_code, 200);
            assert!(article.content.contains(mock_content));
            assert!(article.timestamp <= chrono::Utc::now());

            Ok(())
        }

        #[tokio::test]
        async fn test_error_handling() -> Result<(), anyhow::Error> {
            use crate::infra::api::firecrawl::MockFirecrawlClient;

            let error_client = MockFirecrawlClient::new_error("Network timeout");
            let result = get_article_content_with_client("https://test.com", &error_client).await;

            assert!(result.is_ok(), "エラークライアントでも結果を返すべき");
            let article = result.unwrap();
            assert_eq!(
                article.status_code, 500,
                "エラー時はstatus_code=500になるべき"
            );
            assert!(
                article.content.contains("記事取得APIエラー"),
                "エラー内容が記録されるべき"
            );
            assert!(
                article.content.contains("Network timeout"),
                "具体的なエラーメッセージが含まれるべき"
            );

            Ok(())
        }

        #[tokio::test]
        async fn test_empty_content_handling() -> Result<(), anyhow::Error> {
            use crate::infra::api::firecrawl::MockFirecrawlClient;

            let empty_client = MockFirecrawlClient::new_success("");
            let article =
                get_article_content_with_client("https://empty.test.com", &empty_client).await?;

            assert_eq!(article.status_code, 200);
            assert!(article.content.is_empty());
            assert_eq!(article.url, "https://empty.test.com");

            Ok(())
        }

        #[tokio::test]
        async fn test_url_validation() -> Result<(), anyhow::Error> {
            use crate::infra::api::firecrawl::MockFirecrawlClient;

            let test_urls = [
                "https://example.com",
                "http://test.org/path",
                "https://sub.domain.co.jp/article/123",
                "",            // 空URL
                "invalid-url", // 不正URL
            ];

            for url in test_urls {
                let mock_client = MockFirecrawlClient::new_success("content");
                let result = get_article_content_with_client(url, &mock_client).await;

                assert!(result.is_ok(), "URL '{}' でエラーが発生", url);
                let article = result.unwrap();
                assert_eq!(article.url, url, "URLが正しく設定されていない");
            }

            Ok(())
        }
    }

    mod store_article_content {
        use super::*;

        #[sqlx::test]
        async fn test_basic_storage(pool: PgPool) -> Result<(), anyhow::Error> {
            let now = Utc::now();
            let test_article = ArticleContent {
                url: "https://test.example.com/basic".to_string(),
                timestamp: now,
                status_code: 200,
                content: "# Basic Test Article\n\nThis is basic test content.".to_string(),
            };
            store_article_content(&test_article, &pool).await?;

            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, Some(1), "期待する件数(1件)が保存されませんでした");

            // 保存されたデータの検証
            let stored = sqlx::query_as!(
                ArticleContent,
                "SELECT url, timestamp, status_code, content FROM articles WHERE url = $1",
                test_article.url
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(stored.url, test_article.url);
            assert_eq!(stored.status_code, test_article.status_code);
            assert_eq!(stored.content, test_article.content);

            Ok(())
        }

        #[sqlx::test]
        async fn test_duplicate_handling(pool: PgPool) -> Result<(), anyhow::Error> {
            let now = Utc::now();
            let original_article = ArticleContent {
                url: "https://test.example.com/duplicate".to_string(),
                timestamp: now,
                status_code: 200,
                content: "Original content".to_string(),
            };
            store_article_content(&original_article, &pool).await?;

            let updated_article = ArticleContent {
                url: "https://test.example.com/duplicate".to_string(),
                timestamp: now,
                status_code: 404,
                content: "Updated content".to_string(),
            };
            store_article_content(&updated_article, &pool).await?;

            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(
                count,
                Some(1),
                "重複記事が挿入され、件数が変わってしまいました"
            );

            // 更新されたデータの検証
            let stored = sqlx::query_as!(
                ArticleContent,
                "SELECT url, timestamp, status_code, content FROM articles WHERE url = $1",
                updated_article.url
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(stored.status_code, 404, "status_codeが更新されていない");
            assert_eq!(
                stored.content, "Updated content",
                "contentが更新されていない"
            );

            Ok(())
        }

        #[sqlx::test]
        async fn test_large_content_storage(pool: PgPool) -> Result<(), anyhow::Error> {
            let large_content = "A".repeat(100000); // 100KB のコンテンツ
            let test_article = ArticleContent {
                url: "https://test.example.com/large".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: large_content.clone(),
            };

            store_article_content(&test_article, &pool).await?;

            let stored = sqlx::query_as!(
                ArticleContent,
                "SELECT url, timestamp, status_code, content FROM articles WHERE url = $1",
                test_article.url
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(
                stored.content.len(),
                large_content.len(),
                "大容量コンテンツが正しく保存されていない"
            );
            assert_eq!(
                stored.content, large_content,
                "大容量コンテンツの内容が一致しない"
            );

            Ok(())
        }

        #[sqlx::test]
        async fn test_special_characters(pool: PgPool) -> Result<(), anyhow::Error> {
            let special_content = "特殊文字テスト: éñüñ, 🚀, \"quotes\", <tags>, & entities";
            let test_article = ArticleContent {
                url: "https://test.example.com/special-chars".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: special_content.to_string(),
            };

            store_article_content(&test_article, &pool).await?;

            let stored = sqlx::query_as!(
                ArticleContent,
                "SELECT url, timestamp, status_code, content FROM articles WHERE url = $1",
                test_article.url
            )
            .fetch_one(&pool)
            .await?;

            assert_eq!(
                stored.content, special_content,
                "特殊文字が正しく保存されていない"
            );

            Ok(())
        }

        #[sqlx::test]
        async fn test_no_update_when_same_content(pool: PgPool) -> Result<(), anyhow::Error> {
            let test_article = ArticleContent {
                url: "https://test.example.com/same".to_string(),
                timestamp: Utc::now(),
                status_code: 200,
                content: "Same content".to_string(),
            };

            // 最初の保存
            store_article_content(&test_article, &pool).await?;
            let first_timestamp = sqlx::query_scalar!(
                "SELECT timestamp FROM articles WHERE url = $1",
                test_article.url
            )
            .fetch_one(&pool)
            .await?;

            // 同じ内容で再保存（timestampは異なるがstatus_codeとcontentは同じ）
            std::thread::sleep(std::time::Duration::from_millis(10)); // 時間差を作る
            store_article_content(&test_article, &pool).await?;

            let second_timestamp = sqlx::query_scalar!(
                "SELECT timestamp FROM articles WHERE url = $1",
                test_article.url
            )
            .fetch_one(&pool)
            .await?;

            // 同一内容の場合、timestampが更新されないことを確認
            assert_eq!(
                first_timestamp, second_timestamp,
                "同一内容なのにtimestampが更新された"
            );

            Ok(())
        }
    }

    mod search_article_join_rows {
        use super::*;
        use chrono::Datelike;

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_basic_search(pool: PgPool) -> Result<(), anyhow::Error> {
            let result = search_article_join_rows(None, &pool).await?;
            assert!(result.len() >= 1, "基本検索で最低1件の結果が必要");

            // 日付順ソート（DESC）の確認
            for i in 1..result.len() {
                assert!(
                    result[i - 1].pub_date >= result[i].pub_date,
                    "結果が日付順（降順）にソートされていない"
                );
            }

            Ok(())
        }

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_status_filtering(pool: PgPool) -> Result<(), anyhow::Error> {
            // 成功記事のみ取得（service_basicには200のarticle1,article2がある）
            let success_query = ArticleJoinRowQuery {
                statuses: Some(vec![ArticleStatus::Success]),
                ..Default::default()
            };
            let success_results = search_article_join_rows(Some(success_query), &pool).await?;

            assert!(success_results.len() >= 2, "成功記事が期待件数取得できない");
            for result in &success_results {
                assert_eq!(
                    result.status_code,
                    Some(200),
                    "Success状態でstatus_code=200以外が含まれている"
                );
                assert!(result.content.is_some(), "Success状態でcontentがNone");
            }

            // エラー記事のみ取得（service_basicには404のarticle3がある）
            let error_query = ArticleJoinRowQuery {
                statuses: Some(vec![ArticleStatus::Error(404)]),
                ..Default::default()
            };
            let error_results = search_article_join_rows(Some(error_query), &pool).await?;

            assert!(error_results.len() >= 1, "エラー記事が期待件数取得できない");
            for result in &error_results {
                assert_eq!(
                    result.status_code,
                    Some(404),
                    "エラー状態で期待外のstatus_codeが含まれている: {:?}",
                    result.status_code
                );
            }

            // 未処理記事のみ取得（service_basicではarticle_linksのみで記事が未取得のものを探す）
            let unprocessed_query = ArticleJoinRowQuery {
                statuses: Some(vec![ArticleStatus::Unprocessed]),
                ..Default::default()
            };
            let unprocessed_results =
                search_article_join_rows(Some(unprocessed_query), &pool).await?;

            // service_basicにはarticle_linksが3件、articlesが3件あるので未処理はない可能性
            for result in &unprocessed_results {
                assert!(
                    result.status_code.is_none(),
                    "未処理状態でstatus_codeが存在する"
                );
                assert!(result.content.is_none(), "未処理状態でcontentが存在する");
            }

            Ok(())
        }

        #[sqlx::test(fixtures("service_search_patterns"))]
        async fn test_pattern_and_date_filtering(pool: PgPool) -> Result<(), anyhow::Error> {
            use chrono::{TimeZone, Utc};

            let pub_date_from = Utc.with_ymd_and_hms(2025, 1, 10, 0, 0, 0).unwrap();
            let pub_date_to = Utc.with_ymd_and_hms(2025, 1, 12, 23, 59, 59).unwrap();

            let query = ArticleJoinRowQuery {
                link_pattern: Some("tech.example.com".to_string()),
                pub_date_from: Some(pub_date_from),
                pub_date_to: Some(pub_date_to),
                statuses: None,
                source: None,
                limit: None,
            };

            let results = search_article_join_rows(Some(query), &pool).await?;

            for result in &results {
                assert!(
                    result.url.contains("tech.example.com"),
                    "URLパターンが一致しない: {}",
                    result.url
                );
                assert!(
                    result.pub_date >= pub_date_from,
                    "pub_date_fromより前の日付が含まれている"
                );
                assert!(
                    result.pub_date <= pub_date_to,
                    "pub_date_toより後の日付が含まれている"
                );
            }

            // tech.example.comで該当期間の記事が2件以上あることを確認
            assert!(results.len() >= 2, "期待される件数が取得できていない");

            Ok(())
        }

        #[sqlx::test(fixtures("service_limit_tests"))]
        async fn test_limit_and_source_filtering(pool: PgPool) -> Result<(), anyhow::Error> {
            // limitテスト
            let limit_query = ArticleJoinRowQuery {
                limit: Some(3),
                ..Default::default()
            };
            let limit_results = search_article_join_rows(Some(limit_query), &pool).await?;
            assert!(
                limit_results.len() <= 3,
                "limit=3で3件を超える結果が返された"
            );

            // sourceフィルタテスト
            let source_query = ArticleJoinRowQuery {
                source: Some("tech-feed".to_string()),
                ..Default::default()
            };
            let source_results = search_article_join_rows(Some(source_query), &pool).await?;

            for result in &source_results {
                assert_eq!(
                    result.source, "tech-feed",
                    "source='tech-feed'で異なるsourceが含まれている"
                );
            }

            // 複合条件テスト
            let complex_query = ArticleJoinRowQuery {
                source: Some("limit-feed".to_string()),
                statuses: Some(vec![ArticleStatus::Success]),
                limit: Some(5),
                ..Default::default()
            };
            let complex_results = search_article_join_rows(Some(complex_query), &pool).await?;

            assert!(
                complex_results.len() <= 5,
                "複合条件でlimit=5を超える結果が返された"
            );
            for result in &complex_results {
                assert_eq!(result.source, "limit-feed");
                assert_eq!(result.status_code, Some(200));
            }

            Ok(())
        }

        #[sqlx::test(fixtures("service_boundary_values"))]
        async fn test_edge_cases_and_boundary_values(pool: PgPool) -> Result<(), anyhow::Error> {
            use chrono::{TimeZone, Utc};

            // 空パターンテスト
            let empty_pattern_query = ArticleJoinRowQuery {
                link_pattern: Some("".to_string()),
                ..Default::default()
            };
            let empty_results = search_article_join_rows(Some(empty_pattern_query), &pool).await?;
            // 空パターンは全てにマッチするはず
            let all_results = search_article_join_rows(None, &pool).await?;
            assert_eq!(
                empty_results.len(),
                all_results.len(),
                "空パターンで全件取得されない"
            );

            // 存在しないパターンテスト
            let nonexistent_query = ArticleJoinRowQuery {
                link_pattern: Some("nonexistent.domain.com".to_string()),
                ..Default::default()
            };
            let nonexistent_results =
                search_article_join_rows(Some(nonexistent_query), &pool).await?;
            assert_eq!(
                nonexistent_results.len(),
                0,
                "存在しないパターンで結果が返された"
            );

            // 境界値日付テスト
            let boundary_start = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
            let boundary_end = Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap();

            let boundary_query = ArticleJoinRowQuery {
                pub_date_from: Some(boundary_start),
                pub_date_to: Some(boundary_end),
                ..Default::default()
            };
            let boundary_results = search_article_join_rows(Some(boundary_query), &pool).await?;

            // 境界値に該当する記事があることを確認
            let has_start_boundary = boundary_results.iter().any(|r| {
                r.pub_date.year() == 2025 && r.pub_date.month() == 1 && r.pub_date.day() == 1
            });
            let has_end_boundary = boundary_results.iter().any(|r| {
                r.pub_date.year() == 2025 && r.pub_date.month() == 12 && r.pub_date.day() == 31
            });

            assert!(has_start_boundary, "年始境界値の記事が含まれていない");
            assert!(has_end_boundary, "年末境界値の記事が含まれていない");

            // limit=0テスト
            let zero_limit_query = ArticleJoinRowQuery {
                limit: Some(0),
                ..Default::default()
            };
            let zero_results = search_article_join_rows(Some(zero_limit_query), &pool).await?;
            assert_eq!(zero_results.len(), 0, "limit=0で結果が返された");

            Ok(())
        }
    }

    mod search_article_url_statuses {
        use super::*;

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_basic_url_status_search(pool: PgPool) -> Result<(), anyhow::Error> {
            let results = search_article_url_statuses(None, &pool).await?;
            assert!(results.len() >= 3, "基本検索で十分な件数が取得されていない");

            // URL順ソートの確認
            for i in 1..results.len() {
                assert!(
                    results[i - 1].url <= results[i].url,
                    "結果がURL順にソートされていない"
                );
            }

            Ok(())
        }

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_status_filtering(pool: PgPool) -> Result<(), anyhow::Error> {
            // 成功記事のみ取得
            let success_query = ArticleUrlStatusQuery {
                statuses: Some(vec![ArticleStatus::Success]),
                ..Default::default()
            };
            let success_results = search_article_url_statuses(Some(success_query), &pool).await?;

            for result in &success_results {
                assert_eq!(
                    result.status_code,
                    Some(200),
                    "Success状態で200以外が含まれている"
                );
            }

            // エラー状態の取得（service_basicには404の記事がある）
            let error_query = ArticleUrlStatusQuery {
                statuses: Some(vec![ArticleStatus::Error(404)]),
                ..Default::default()
            };
            let error_results = search_article_url_statuses(Some(error_query), &pool).await?;

            for result in &error_results {
                assert_eq!(
                    result.status_code,
                    Some(404),
                    "エラー状態で期待外のstatus_codeが含まれている: {:?}",
                    result.status_code
                );
            }

            // 未処理記事のみ取得（service_basicではarticle_linksのみで記事が未取得のものを探す）
            let unprocessed_query = ArticleUrlStatusQuery {
                statuses: Some(vec![ArticleStatus::Unprocessed]),
                ..Default::default()
            };
            let unprocessed_results =
                search_article_url_statuses(Some(unprocessed_query), &pool).await?;

            // service_basicでは全てのarticle_linksに対応するarticlesがある可能性があるので、
            // 未処理はないかもしれません
            for result in &unprocessed_results {
                assert!(
                    result.status_code.is_none(),
                    "未処理状態でstatus_codeが存在する"
                );
            }

            Ok(())
        }

        #[sqlx::test(fixtures("service_search_patterns"))]
        async fn test_url_pattern_filtering(pool: PgPool) -> Result<(), anyhow::Error> {
            // 特定ドメインの検索
            let domain_query = ArticleUrlStatusQuery {
                url_pattern: Some("example.com".to_string()),
                ..Default::default()
            };
            let domain_results = search_article_url_statuses(Some(domain_query), &pool).await?;

            for result in &domain_results {
                assert!(
                    result.url.contains("example.com"),
                    "URLパターンが一致しない: {}",
                    result.url
                );
            }

            // サブドメインの検索
            let subdomain_query = ArticleUrlStatusQuery {
                url_pattern: Some("tech.example.com".to_string()),
                ..Default::default()
            };
            let subdomain_results =
                search_article_url_statuses(Some(subdomain_query), &pool).await?;

            for result in &subdomain_results {
                assert!(
                    result.url.contains("tech.example.com"),
                    "サブドメインパターンが一致しない: {}",
                    result.url
                );
            }

            // パス部分の検索
            let path_query = ArticleUrlStatusQuery {
                url_pattern: Some("tutorial".to_string()),
                ..Default::default()
            };
            let path_results = search_article_url_statuses(Some(path_query), &pool).await?;

            for result in &path_results {
                assert!(
                    result.url.contains("tutorial"),
                    "パスパターンが一致しない: {}",
                    result.url
                );
            }

            Ok(())
        }
    }

    mod search_article_contents {
        use super::*;

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_basic_content_search(pool: PgPool) -> Result<(), anyhow::Error> {
            let results = search_article_contents(None, &pool).await?;
            assert!(
                results.len() >= 2,
                "基本検索で十分な記事内容が取得されていない"
            );

            // timestamp順ソート（DESC）の確認
            for i in 1..results.len() {
                assert!(
                    results[i - 1].timestamp >= results[i].timestamp,
                    "結果がtimestamp順（降順）にソートされていない"
                );
            }

            Ok(())
        }

        #[sqlx::test(fixtures("service_search_patterns"))]
        async fn test_url_pattern_search(pool: PgPool) -> Result<(), anyhow::Error> {
            let query = ArticleContentQuery {
                url_pattern: Some("tech.example.com".to_string()),
                ..Default::default()
            };
            let results = search_article_contents(Some(query), &pool).await?;

            for result in &results {
                assert!(
                    result.url.contains("tech.example.com"),
                    "URLパターンが一致しない"
                );
                assert!(result.status_code > 0, "status_codeが無効");
                assert!(
                    !result.content.is_empty() || result.status_code != 200,
                    "成功記事でcontentが空"
                );
            }

            Ok(())
        }

        #[sqlx::test(fixtures("service_search_patterns"))]
        async fn test_timestamp_range_filtering(pool: PgPool) -> Result<(), anyhow::Error> {
            use chrono::{TimeZone, Utc};

            let timestamp_from = Utc.with_ymd_and_hms(2025, 1, 10, 0, 0, 0).unwrap();
            let timestamp_to = Utc.with_ymd_and_hms(2025, 1, 12, 23, 59, 59).unwrap();

            let query = ArticleContentQuery {
                timestamp_from: Some(timestamp_from),
                timestamp_to: Some(timestamp_to),
                ..Default::default()
            };
            let results = search_article_contents(Some(query), &pool).await?;

            for result in &results {
                assert!(
                    result.timestamp >= timestamp_from,
                    "timestamp_fromより前のtimestamp"
                );
                assert!(
                    result.timestamp <= timestamp_to,
                    "timestamp_toより後のtimestamp"
                );
            }

            Ok(())
        }

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_status_code_filtering(pool: PgPool) -> Result<(), anyhow::Error> {
            // 成功記事のみ
            let success_query = ArticleContentQuery {
                status_code: Some(200),
                ..Default::default()
            };
            let success_results = search_article_contents(Some(success_query), &pool).await?;

            for result in &success_results {
                assert_eq!(result.status_code, 200, "status_code=200以外が含まれている");
                // 成功記事でも空コンテンツは許可（empty.example.com/titleのケース）
                // assert!(!result.content.is_empty(), "成功記事でcontentが空");
            }

            // エラー記事のみ
            let error_query = ArticleContentQuery {
                status_code: Some(404),
                ..Default::default()
            };
            let error_results = search_article_contents(Some(error_query), &pool).await?;

            for result in &error_results {
                assert_eq!(result.status_code, 404, "status_code=404以外が含まれている");
            }

            Ok(())
        }
    }

    mod called {
        use super::*;

        #[sqlx::test]
        async fn test_search_article_contents(pool: PgPool) -> Result<(), anyhow::Error> {
            let now = Utc::now();
            let test_article = ArticleContent {
                url: "https://search.test.com/article".to_string(),
                timestamp: now,
                status_code: 200,
                content: "検索テスト記事".to_string(),
            };
            store_article_content(&test_article, &pool).await?;

            let query = ArticleContentQuery {
                url_pattern: Some("search.test.com".to_string()),
                ..Default::default()
            };
            let results = search_article_contents(Some(query), &pool).await?;
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].url, "https://search.test.com/article");

            println!("✅ 記事検索テスト成功");
            Ok(())
        }

        #[sqlx::test]
        async fn test_fetch_and_store_article_with_mock(pool: PgPool) -> Result<(), anyhow::Error> {
            use crate::infra::api::firecrawl::MockFirecrawlClient;

            let test_url = "https://integrate.test.com/article";
            let mock_content = "統合テスト記事内容";
            let mock_client = MockFirecrawlClient::new_success(mock_content);

            let article =
                fetch_and_store_article_with_client(test_url, &mock_client, &pool).await?;

            assert_eq!(article.url, test_url);
            assert!(article.content.contains(mock_content));

            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM articles")
                .fetch_one(&pool)
                .await?;
            assert_eq!(count, Some(1));

            println!("✅ 統合機能テスト成功");
            Ok(())
        }

        #[sqlx::test]
        async fn test_search_article_join_rows(pool: PgPool) -> Result<(), anyhow::Error> {
            // テストデータを挿入
            sqlx::query!(
                "INSERT INTO article_links (url, title, pub_date, source) VALUES ($1, $2, $3, $4)",
                "https://test.com/join",
                "テスト記事",
                chrono::Utc::now(),
                "test"
            )
            .execute(&pool)
            .await?;

            sqlx::query!(
                "INSERT INTO articles (url, status_code, content) VALUES ($1, $2, $3)",
                "https://test.com/join",
                200,
                "テスト内容"
            )
            .execute(&pool)
            .await?;

            let query = ArticleJoinRowQuery {
                statuses: Some(vec![ArticleStatus::Success]),
                ..Default::default()
            };
            let results = search_article_join_rows(Some(query), &pool).await?;

            assert!(!results.is_empty());
            let result = &results[0];
            assert_eq!(result.url, "https://test.com/join");
            assert_eq!(result.status_code, Some(200));
            assert!(result.content.is_some());

            println!("✅ ArticleJoinRow検索テスト成功");
            Ok(())
        }

        #[sqlx::test]
        async fn test_search_article_url_statuses(pool: PgPool) -> Result<(), anyhow::Error> {
            // テストデータを挿入
            sqlx::query!(
                "INSERT INTO article_links (url, title, pub_date, source) VALUES ($1, $2, $3, $4)",
                "https://status.test.com/url1",
                "ステータステスト記事1",
                chrono::Utc::now(),
                "test"
            )
            .execute(&pool)
            .await?;

            sqlx::query!(
                "INSERT INTO article_links (url, title, pub_date, source) VALUES ($1, $2, $3, $4)",
                "https://status.test.com/url2",
                "ステータステスト記事2",
                chrono::Utc::now(),
                "test"
            )
            .execute(&pool)
            .await?;

            sqlx::query!(
                "INSERT INTO articles (url, status_code, content) VALUES ($1, $2, $3)",
                "https://status.test.com/url1",
                404,
                "エラー内容"
            )
            .execute(&pool)
            .await?;

            // エラー状態のURLを検索
            let query = ArticleUrlStatusQuery {
                statuses: Some(vec![ArticleStatus::Error(404)]),
                ..Default::default()
            };
            let results = search_article_url_statuses(Some(query), &pool).await?;

            assert!(!results.is_empty());
            let result = &results[0];
            assert_eq!(result.url, "https://status.test.com/url1");
            assert_eq!(result.status_code, Some(404));

            println!("✅ ArticleUrlStatus検索テスト成功");
            Ok(())
        }
    }

    mod online {
        use super::*;

        #[sqlx::test(fixtures("service_basic"))]
        async fn test_search_article_join_rows_with_fixtures(
            pool: PgPool,
        ) -> Result<(), anyhow::Error> {
            let all_rows = search_article_join_rows(None, &pool).await?;
            assert!(all_rows.len() >= 2, "最低2件の結合結果が取得されるべき");

            let success_query = ArticleJoinRowQuery {
                statuses: Some(vec![ArticleStatus::Success]),
                ..Default::default()
            };
            let success_rows = search_article_join_rows(Some(success_query), &pool).await?;
            let success_count = success_rows
                .iter()
                .filter(|row| row.status_code == Some(200))
                .count();
            assert_eq!(
                success_count,
                success_rows.len(),
                "成功記事のみが取得されるべき"
            );

            println!("✅ フィクスチャでのJOIN検索テスト成功");
            Ok(())
        }
    }
}
