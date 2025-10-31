FROM docker.io/library/rust:1.90 AS build
WORKDIR /app
RUN rustup target add x86_64-unknown-linux-musl
COPY . .
RUN cargo build --target x86_64-unknown-linux-musl --release

FROM scratch
COPY --from=build /app/target/x86_64-unknown-linux-musl/release/internal-smithery-mcp /app/internal-smithery-mcp
EXPOSE 8081
ENTRYPOINT [ "/app/internal-smithery-mcp" ]
