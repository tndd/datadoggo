docker compose down -v
docker compose up -d postgres
cd ../
sqlx migrate run