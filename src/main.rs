pub mod tool;

use rmcp::{
    handler::server::wrapper::Parameters,
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

    /// Fetches a URL from the internet and extracts its contents as markdown.
    /// This is the highly recommended way to fetch pages.
    #[rmcp::tool]
    async fn fetch(
        &self,
        Parameters(tool::fetch::Input { url }): Parameters<tool::fetch::Input>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tool::fetch::fetch(&url).await;

        match result {
            Ok(markdown) => {
                let results = vec![Content::text(markdown)];
                Ok(rmcp::model::CallToolResult::success(results))
            }
            Err(e) => {
                let errors = vec![Content::text(e.to_string())];
                Ok(rmcp::model::CallToolResult::error(errors))
            }
        }
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

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8081").await.unwrap();
    axum::serve(listener, router).await.unwrap();
}
