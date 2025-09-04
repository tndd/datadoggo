docker compose down -v
docker compose up -d postgres
sqlx migrate run --source sql/migrations