FROM rust:1.80

# PostgreSQLライブラリをインストール
RUN apt-get update && apt-get install -y libpq-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release

ENV DATABASE_URL="host=postgres user=datadoggo password=datadoggo dbname=datadoggo"
CMD ["cargo", "run", "--release"]