# ========================================
# datadoggo プロジェクト justfile
# ========================================
#
# 主要コマンド:
#   just test                    - 包括的テスト実行（品質チェック + テスト）
#   just setup [env] [--clear]   - 環境セットアップ（デフォルト: test）
#   just lint                    - コード品質チェック（フォーマット → チェック → リント）
#
# 環境:
#   test  - テスト環境（デフォルト）
#   prod  - 本番環境
#   all   - 両方の環境
#
# フラグ:
#   --clear  - コンテナを削除して再作成（本番環境では確認プロンプト表示）
#

set dotenv-load

# ========================================
# ヘルパー関数
# ========================================

# 本番環境かどうかを判定
[private]
is_production_env env:
    #!/usr/bin/env bash
    [ "{{env}}" = "prod" ] || [ "{{env}}" = "all" ]

# PostgreSQLコンテナの起動完了を待機
[private]
wait_for_postgres container:
    #!/usr/bin/env bash
    set -euo pipefail

    max_attempts=30
    attempt=1

    echo "Waiting for {{container}} to be ready..."
    while ! docker compose exec {{container}} pg_isready -U datadoggo >/dev/null 2>&1; do
        if [ $attempt -ge $max_attempts ]; then
            echo "Error: {{container}} failed to start within ${max_attempts} seconds"
            exit 1
        fi
        echo -n "."
        sleep 1
        attempt=$((attempt + 1))
    done
    echo " ✓ {{container}} ready!"

# コンテナが既に起動しているかチェック
[private]
is_container_running container:
    #!/usr/bin/env bash
    set -euo pipefail

    # コンテナの状態をチェック（Up状態かどうか）
    if docker compose ps {{container}} | grep -q "Up"; then
        echo "{{container}} is already running"
        exit 0
    else
        exit 1
    fi

# setup用の引数解析
[private]
parse_setup_args env *flags:
    #!/usr/bin/env bash
    set -euo pipefail

    # 引数解析結果を環境変数として出力
    if [ "{{env}}" = "--clear" ]; then
        echo "ENV_NAME=test"
        echo "CLEAR_FLAG=true"
    else
        env_name="{{env}}"
        clear_flag="false"

        for flag in {{flags}}; do
            if [ "$flag" = "--clear" ]; then
                clear_flag="true"
                break
            fi
        done

        echo "ENV_NAME=$env_name"
        echo "CLEAR_FLAG=$clear_flag"
    fi

# 本番環境削除時の確認プロンプト
[private]
confirm_production_deletion env:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "警告: 本番環境のデータベースを削除しようとしています。"
    echo "環境: {{env}}"
    read -p "続行しますか？ (y/N): " confirm
    if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
        echo "操作をキャンセルしました。"
        exit 1
    fi

# コンテナ削除処理
[private]
remove_containers env:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{env}}" = "prod" ] || [ "{{env}}" = "all" ]; then
        docker compose down postgres -v
    fi
    if [ "{{env}}" = "test" ] || [ "{{env}}" = "all" ]; then
        docker compose down postgres-test -v
    fi

# コンテナ起動処理（インテリジェント待機）
[private]
start_containers env:
    #!/usr/bin/env bash
    set -euo pipefail

    if [ "{{env}}" = "prod" ]; then
        if just is_container_running postgres; then
            just wait_for_postgres postgres
        else
            docker compose up -d postgres
            just wait_for_postgres postgres
        fi
    elif [ "{{env}}" = "test" ]; then
        if just is_container_running postgres-test; then
            just wait_for_postgres postgres-test
        else
            docker compose up -d postgres-test
            just wait_for_postgres postgres-test
        fi
    elif [ "{{env}}" = "all" ]; then
        # 並列起動
        docker compose up -d postgres postgres-test

        # 並列待機
        (just wait_for_postgres postgres) &
        (just wait_for_postgres postgres-test) &
        wait  # 両方の完了を待機
    else
        echo "エラー: 無効なenv '{{env}}'. 'prod', 'test', 'all'のいずれかを指定してください。"
        exit 1
    fi

# ========================================
# DBコンテナ管理
# ========================================

# DBコンテナcomposeコマンド
[private]
compose_up env clean="false":
    #!/usr/bin/env bash
    set -euo pipefail

    # クリーンフラグが設定されている場合はコンテナを再作成
    if [ "{{clean}}" = "true" ]; then
        # 本番環境の場合は確認プロンプトを表示
        if just is_production_env {{env}}; then
            just confirm_production_deletion {{env}}
        fi
        just remove_containers {{env}}
    fi

    # 指定されたデータベースコンテナを起動
    just start_containers {{env}}

# マイグレーション実行の共通処理
[private]
migrate env:
    #!/usr/bin/env bash
    set -euo pipefail

    if [ "{{env}}" = "prod" ]; then
        DATABASE_URL="${DATABASE_URL_PROD}" sqlx migrate run
        echo "本番用データベースのマイグレーションが完了しました。"
    elif [ "{{env}}" = "test" ]; then
        sqlx migrate run
        echo "テスト用データベースのマイグレーションが完了しました。"
    elif [ "{{env}}" = "all" ]; then
        sqlx migrate run
        DATABASE_URL="${DATABASE_URL_PROD}" sqlx migrate run
        echo "すべてのデータベースのマイグレーションが完了しました。"
    fi

# ========================================
# 開発ツール
# ========================================


# コード品質チェック
# （compose up + fmt → clippy）
# NOTE: compose upを先にするのはsqlxのため
lint:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "> just compose"
    just compose_up test false

    echo "> cargo fmt"
    # fmt対象箇所表示のため
    if ! cargo fmt --check; then
        cargo fmt
    fi
    echo "> cargo clippy"
    cargo clippy -- -D warnings

# 包括的テスト実行
# (コード品質チェック + テスト実行）
test:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "> just lint"
    just lint  # この時点でcompose upが保証される

    echo "> cargo test"
    cargo test --lib
    echo "Complete: just test"

# ========================================
# メインコマンド
# ========================================

# 環境セットアップ
# env: prod,test,all
# flags: --clearでコンテナ再作成）
setup env="test" *flags="":
    #!/usr/bin/env bash
    set -euo pipefail

    # 引数解析
    eval $(just parse_setup_args {{env}} {{flags}})

    echo "=== ${ENV_NAME}環境のセットアップを開始します ==="
    if ! just compose_up $ENV_NAME $CLEAR_FLAG; then
        echo "セットアップがキャンセルまたは失敗しました。"
        exit 1
    fi
    just migrate $ENV_NAME
    echo "=== ${ENV_NAME}環境のセットアップが完了しました ==="
