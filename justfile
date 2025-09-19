# データベース関連タスク
# - just migrate - テスト用DB（デフォルト）
# - just migrate prod - 本番用DB
# - just migrate both - 両方のDB
# - just migrate test --clean - テスト用DB（クリーン再作成）
# - just setup - 初期セットアップ（両方のDBをクリーン作成）
# - just test - テスト実行（共通処理を利用）

set dotenv-load

# データベース準備の共通処理
[private]
prepare_database db_type clean="false":
    #!/usr/bin/env bash
    set -euo pipefail

    # クリーンフラグが設定されている場合はコンテナを再作成
    if [ "{{clean}}" = "true" ]; then
        if [ "{{db_type}}" = "prod" ] || [ "{{db_type}}" = "both" ]; then
            docker compose down postgres -v
        fi
        if [ "{{db_type}}" = "test" ] || [ "{{db_type}}" = "both" ]; then
            docker compose down postgres-test -v
        fi
    fi

    # 指定されたデータベースコンテナを起動
    if [ "{{db_type}}" = "prod" ]; then
        docker compose up -d postgres
        sleep 5
        docker compose exec postgres pg_isready -U datadoggo
    elif [ "{{db_type}}" = "test" ]; then
        docker compose up -d postgres-test
        sleep 5
        docker compose exec postgres-test pg_isready -U datadoggo
    elif [ "{{db_type}}" = "both" ]; then
        docker compose up -d postgres postgres-test
        sleep 10
        docker compose exec postgres pg_isready -U datadoggo
        docker compose exec postgres-test pg_isready -U datadoggo
    else
        echo "エラー: 無効なdb_type '{{db_type}}'. 'prod', 'test', 'both'のいずれかを指定してください。"
        exit 1
    fi

# マイグレーション実行の共通処理
[private]
run_migration db_type:
    #!/usr/bin/env bash
    set -euo pipefail

    if [ "{{db_type}}" = "prod" ]; then
        DATABASE_URL="${PROD_DB_URL}" sqlx migrate run
        echo "本番用データベースのマイグレーションが完了しました。"
    elif [ "{{db_type}}" = "test" ]; then
        DATABASE_URL="${TEST_DB_URL}" sqlx migrate run
        echo "テスト用データベースのマイグレーションが完了しました。"
    elif [ "{{db_type}}" = "both" ]; then
        DATABASE_URL="${PROD_DB_URL}" sqlx migrate run
        DATABASE_URL="${TEST_DB_URL}" sqlx migrate run
        echo "両方のデータベースのマイグレーションが完了しました。"
    fi

# テスト用データベースでテスト実行
test:
    #!/usr/bin/env bash
    set -euo pipefail
    just prepare_database test false
    DATABASE_URL="${TEST_DB_URL}" cargo test --lib

# データベースマイグレーション実行（デフォルト: テスト用、--cleanでコンテナ再作成）
migrate db_type="test" *flags="":
    #!/usr/bin/env bash
    set -euo pipefail

    # --cleanフラグの確認
    clean="false"
    for flag in {{flags}}; do
        if [ "$flag" = "--clean" ]; then
            clean="true"
            break
        fi
    done

    just prepare_database {{db_type}} $clean
    just run_migration {{db_type}}

# プロジェクトの初期セットアップ（両方のDBをクリーン作成してマイグレーション）
setup:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "プロジェクトの初期セットアップを開始します..."
    just prepare_database both true
    just run_migration both
    echo "初期セットアップが完了しました。"

# sqlx の prepare (オフラインコンパイル用)
prepare:
    DATABASE_URL="${TEST_DB_URL}" cargo sqlx prepare

# コード品質チェック（フォーマット → チェック → リント）
lint:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo fmt
    echo "> cargo check"
    SQLX_OFFLINE=true cargo check
    echo "> cargo clippy"
    cargo clippy -- -D warnings
