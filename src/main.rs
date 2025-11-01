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
        Parameters(tool::fetch::Input { urls }): Parameters<tool::fetch::Input>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tool::fetch::fetch(urls).await;

        match result {
            Ok(markdown_list) => {
                let results = markdown_list
                    .into_iter()
                    .map(|markdown| Content::text(markdown))
                    .collect::<Vec<Content>>();
                Ok(rmcp::model::CallToolResult::success(results))
            }
            Err(e) => {
                let errors = vec![Content::text(e.to_string())];
                Ok(rmcp::model::CallToolResult::error(errors))
            }
        }
    }

    #[rmcp::tool]
    async fn search(
        &self,
        Parameters(tool::search::Input { query }): Parameters<tool::search::Input>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let response = crate::tool::search::search(query).await;

        match response {
            Ok(search_results) => {
                let mut results = vec![];

                for search_result in search_results {
                    let content = serde_json::to_string(&search_result)
                        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

                    results.push(Content::text(content));
                }

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
            instructions: Some("set of utilities".into()),
            capabilities: rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: rmcp::model::Implementation {
                name: "internal-smithery-mcp".to_owned(),
                title: Some("Internal Smithery MCP".to_owned()),
                version: "0.1.0".to_owned(),
                icons: Some(vec![rmcp::model::Icon {
                    src: "https://www.ikuma.cloud/brand/favicon.svg".to_owned(),
                    mime_type: Some("image/svg+xml".to_owned()),
                    sizes: None,
                }]),
                website_url: None,
            },
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

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
