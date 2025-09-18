# データベース関連タスク
set dotenv-load

# テスト用データベースでテスト実行
test:
    #!/usr/bin/env bash
    set -euo pipefail
    docker compose up -d postgres-test
    sleep 5
    docker compose exec postgres-test pg_isready -U datadoggo
    DATABASE_URL="${TEST_DB_URL}" cargo test --lib

# テスト用データベースでマイグレーション実行
migrate:
    #!/usr/bin/env bash
    set -euo pipefail
    docker compose down postgres-test -v
    docker compose up -d postgres-test
    sleep 5
    docker compose exec postgres-test pg_isready -U datadoggo
    DATABASE_URL="${TEST_DB_URL}" sqlx migrate run
    echo "テスト用データベースのマイグレーションが完了しました。"

# 本番用データベースでマイグレーション実行
migrate-prod:
    #!/usr/bin/env bash
    set -euo pipefail
    docker compose down postgres -v
    docker compose up -d postgres
    sleep 5
    docker compose exec postgres pg_isready -U datadoggo
    DATABASE_URL="${PROD_DB_URL}" sqlx migrate run
    echo "本番用データベースのマイグレーションが完了しました。"

# テスト用データベースの完全リセット
reset-db:
    #!/usr/bin/env bash
    set -euo pipefail
    docker compose down postgres-test -v
    docker compose up -d postgres-test
    sleep 5
    docker compose exec postgres-test pg_isready -U datadoggo
    DATABASE_URL="${TEST_DB_URL}" sqlx migrate run
    echo "テスト用データベースをリセットしました。"

# プロジェクトの初期セットアップ
setup:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "プロジェクトの初期セットアップを開始します..."
    docker compose up -d postgres postgres-test
    sleep 10
    docker compose exec postgres pg_isready -U datadoggo
    docker compose exec postgres-test pg_isready -U datadoggo
    DATABASE_URL="${PROD_DB_URL}" sqlx migrate run
    DATABASE_URL="${TEST_DB_URL}" sqlx migrate run
    echo "初期セットアップが完了しました。"

# sqlx の prepare (オフラインコンパイル用)
prepare:
    DATABASE_URL="${TEST_DB_URL}" cargo sqlx prepare

# コードチェック（オフラインモード）
check:
    SQLX_OFFLINE=true cargo check

# linting
lint:
    cargo clippy -- -D warnings

# フォーマット
fmt:
    cargo fmt