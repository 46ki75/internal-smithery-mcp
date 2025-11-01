use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Input {
    /// The natural language query to search for.
    pub query: String,

    /// If specified, results will only come from these domains.
    /// e.g., `["example.como"]`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_domains: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
struct Request {
    pub query: String,
    pub include_domains: Option<Vec<String>>,
    pub num_results: u8,
    pub contents: Contents,
}

#[derive(Debug, Clone, Serialize)]
struct Contents {
    pub text: bool,
    pub summary: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct Response {
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    // pub text: String,
    pub summary: String,
}

pub async fn search(
    query: String,
    include_domains: Option<Vec<String>>,
) -> Result<Vec<SearchResult>, Box<dyn std::error::Error>> {
    let api_key = std::env::var("EXA_API_KEY")?;

    let client = reqwest::Client::new();

    let body = Request {
        query,
        include_domains,
        num_results: 3,
        contents: Contents {
            summary: true,
            text: false,
        },
    };

    let body_string = serde_json::to_string(&body)?;

    let request = client
        .post("https://api.exa.ai/search")
        .header("x-api-key", api_key)
        .header("content-type", "application/json")
        .body(body_string);

    let response = request.send().await?.text().await?;

    let results = serde_json::from_str::<Response>(&response)?.results;

    Ok(results)
}
