# just test - 包括的テスト実行（品質チェック + テスト）
# just setup [env] [--clear] - 環境セットアップ（デフォルト: test）
# just lint - コード品質チェック（フォーマット → チェック → リント）

set dotenv-load

# DBコンテナcomposeコマンド
[private]
compose_up env clean="false":
    #!/usr/bin/env bash
    set -euo pipefail

    # クリーンフラグが設定されている場合はコンテナを再作成
    if [ "{{clean}}" = "true" ]; then
        # 本番環境削除時の確認プロンプト
        if [ "{{env}}" = "prod" ] || [ "{{env}}" = "all" ]; then
            echo "警告: 本番環境のデータベースを削除しようとしています。"
            echo "環境: {{env}}"
            read -p "続行しますか？ (y/N): " confirm
            if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
                echo "操作をキャンセルしました。"
                exit 1
            fi
        fi

        if [ "{{env}}" = "prod" ] || [ "{{env}}" = "all" ]; then
            docker compose down postgres -v
        fi
        if [ "{{env}}" = "test" ] || [ "{{env}}" = "all" ]; then
            docker compose down postgres-test -v
        fi
    fi

    # 指定されたデータベースコンテナを起動
    if [ "{{env}}" = "prod" ]; then
        docker compose up -d postgres
        sleep 5
        docker compose exec postgres pg_isready -U datadoggo
    elif [ "{{env}}" = "test" ]; then
        docker compose up -d postgres-test
        sleep 5
        docker compose exec postgres-test pg_isready -U datadoggo
    elif [ "{{env}}" = "all" ]; then
        docker compose up -d postgres postgres-test
        sleep 10
        docker compose exec postgres pg_isready -U datadoggo
        docker compose exec postgres-test pg_isready -U datadoggo
    else
        echo "エラー: 無効なenv '{{env}}'. 'prod', 'test', 'all'のいずれかを指定してください。"
        exit 1
    fi

# マイグレーション実行の共通処理
[private]
migrate env:
    #!/usr/bin/env bash
    set -euo pipefail

    if [ "{{env}}" = "prod" ]; then
        DATABASE_URL="${PROD_DB_URL}" sqlx migrate run
        echo "本番用データベースのマイグレーションが完了しました。"
    elif [ "{{env}}" = "test" ]; then
        DATABASE_URL="${TEST_DB_URL}" sqlx migrate run
        echo "テスト用データベースのマイグレーションが完了しました。"
    elif [ "{{env}}" = "all" ]; then
        DATABASE_URL="${PROD_DB_URL}" sqlx migrate run
        DATABASE_URL="${TEST_DB_URL}" sqlx migrate run
        echo "すべてのデータベースのマイグレーションが完了しました。"
    fi

# sqlx prepare実行の共通処理
sqlx_prepare:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "> cargo sqlx prepare"
    DATABASE_URL="${TEST_DB_URL}" cargo sqlx prepare

# コード品質チェック
# （フォーマット → チェック → リント）
lint:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "> cargo fmt"
    # fmt対象箇所表示のため
    if ! cargo fmt --check; then
        cargo fmt
    fi
    echo "> cargo check"
    SQLX_OFFLINE=true cargo check --all --locked
    echo "> cargo clippy"
    cargo clippy -- -D warnings

# 包括的テスト実行
# (コード品質チェック + compose up + テスト実行）
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

# 環境セットアップ
# env: prod,test,all
# flags: --clearでコンテナ再作成）
setup env="test" *flags="":
    #!/usr/bin/env bash
    set -euo pipefail

    # 引数解析: 最初の引数が--clearの場合の処理
    if [ "{{env}}" = "--clear" ]; then
        actual_env="test"
        clear="true"
    else
        actual_env="{{env}}"
        clear="false"
        for flag in {{flags}}; do
            if [ "$flag" = "--clear" ]; then
                clear="true"
                break
            fi
        done
    fi

    echo "=== ${actual_env}環境のセットアップを開始します ==="
    if ! just compose_up $actual_env $clear; then
        echo "セットアップがキャンセルまたは失敗しました。"
        exit 1
    fi
    just migrate $actual_env
    echo "=== ${actual_env}環境のセットアップが完了しました ==="
