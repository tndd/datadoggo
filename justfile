# just test - 包括的テスト実行（品質チェック + テスト）
# just setup [env] [--clear] - 環境セットアップ（デフォルト: both）
# just lint - コード品質チェック（フォーマット → チェック → リント）

set dotenv-load

# DBコンテナcomposeコマンド
[private]
compose_up db_type clean="false":
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
migrate db_type:
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

# sqlx prepare実行の共通処理
sqlx_prepare:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "> cargo sqlx prepare"
    DATABASE_URL="${TEST_DB_URL}" cargo sqlx prepare

# コード品質チェック（フォーマット → チェック → リント）
lint:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "> cargo fmt"
    cargo fmt
    echo "> cargo check"
    SQLX_OFFLINE=true cargo check --all --locked
    echo "> cargo clippy"
    cargo clippy -- -D warnings

# 包括的テスト実行（コード品質チェック + sqlx prepare + テスト実行）
test:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "> just lint"
    just lint

    echo "> just compose"
    just compose_up test false

    echo "> cargo test"
    DATABASE_URL="${TEST_DB_URL}" cargo test --lib
    echo "Complete: just test"

# 環境セットアップ（env: prod/test/both, --clearでコンテナ再作成）
setup env="both" *flags="":
    #!/usr/bin/env bash
    set -euo pipefail

    # --clearフラグの確認
    clear="false"
    for flag in {{flags}}; do
        if [ "$flag" = "--clear" ]; then
            clear="true"
            break
        fi
    done

    echo "=== {{env}}環境のセットアップを開始します ==="
    just compose_up {{env}} $clear
    just migrate {{env}}
    echo "=== {{env}}環境のセットアップが完了しました ==="
