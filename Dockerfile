FROM docker.io/library/rust:1.90 AS build
WORKDIR /app
COPY . .
RUN cargo build --release
EXPOSE 8081
ENTRYPOINT [ "/app/target/release/internal-smithery-mcp" ]
