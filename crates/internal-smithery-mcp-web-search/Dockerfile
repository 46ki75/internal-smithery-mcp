FROM docker.io/library/rust:1.90 AS build
WORKDIR /app
RUN apt-get update -y && \
    apt-get install -y npm && \
    npx -y @puppeteer/browsers install chrome-headless-shell@stable --install-deps --path /opt && \
    ln -s $(ls /opt/chrome-headless-shell/linux-*/chrome-headless-shell-linux64/chrome-headless-shell) /bin/chrome-headless-shell && \
    apt-get remove -y npm && \
    apt-get autoremove -y && \
    rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release
EXPOSE 8081
ENTRYPOINT [ "/app/target/release/internal-smithery-mcp-web-search" ]
