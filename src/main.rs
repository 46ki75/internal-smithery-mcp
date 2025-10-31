use rmcp::{
    model::{CallToolResult, Content},
    transport::{
        StreamableHttpServerConfig, StreamableHttpService,
        streamable_http_server::session::local::LocalSessionManager,
    },
};

#[derive(Debug, Clone)]
pub struct Counter {
    tool_router: rmcp::handler::server::tool::ToolRouter<Self>,
}

#[rmcp::tool_router]
impl Counter {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[rmcp::tool(description = "greeting")]
    async fn greet(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(rmcp::model::CallToolResult::success(vec![Content::text(
            "Hello.".to_string(),
        )]))
    }
}

#[rmcp::tool_handler]
impl rmcp::ServerHandler for Counter {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        rmcp::model::ServerInfo {
            instructions: Some("A simple calculator".into()),
            capabilities: rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() {
    let service = StreamableHttpService::new(
        || Ok(crate::Counter::new()),
        std::sync::Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig {
            stateful_mode: false,
            ..Default::default()
        },
    );

    let router: axum::Router = axum::Router::new().nest_service("/mcp", service);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:12000")
        .await
        .unwrap();
    axum::serve(listener, router).await.unwrap();
}
