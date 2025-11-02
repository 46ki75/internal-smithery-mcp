use std::{path::PathBuf, time::Duration};

use headless_chrome::Tab;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Input {
    /// A list of URLs to fetch.
    pub urls: Vec<String>,
}

/// Minimum content length threshold to consider content sufficient
const MIN_CONTENT_LENGTH: usize = 300;

/// Process HTML to markdown and validate content sufficiency
fn process_html(html: &str) -> (String, bool) {
    let markdown = html2md::rewrite_html(html, false);
    let is_sufficient = markdown.trim().len() >= MIN_CONTENT_LENGTH;
    (markdown, is_sufficient)
}

struct FlexibleWaiter<'a> {
    tab: &'a Tab,
    timeout: Duration,
}

impl<'a> FlexibleWaiter<'a> {
    fn new(tab: &'a Tab) -> Self {
        Self {
            tab,
            timeout: Duration::from_secs(30),
        }
    }

    fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn wait_smart(&self) -> Result<(), Box<dyn std::error::Error + Send>> {
        let start = std::time::Instant::now();

        let common_selectors = vec![
            "main",
            "article",
            "[role='main']",
            ".content",
            ".main-content",
            "#content",
            "[data-testid]",
            "[data-component]",
        ];

        while start.elapsed() < self.timeout {
            for selector in &common_selectors {
                if self.tab.find_element(selector).is_ok() {
                    tracing::info!("Found element with selector: {}", selector);
                    return Ok(());
                }
            }

            let has_content = self
                .tab
                .evaluate(
                    r#"
                // Check whether the body has sufficient content
                document.body.innerText.length > 100 &&
                // Check for a minimal DOM structure
                document.body.children.length > 0
                "#,
                    false,
                )?
                .value
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if has_content {
                tracing::info!("Found content by checking body");
                return Ok(());
            }

            std::thread::sleep(Duration::from_millis(50));
        }

        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "Timeout: No suitable element found",
        )))
    }
}

async fn fetch_with_reqwest(url: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let html = response
        .text()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let (markdown, is_sufficient) = process_html(&html);

    if is_sufficient {
        tracing::info!("Successfully fetched with reqwest: {}", url);
        Ok(format!("<{url}>\n\n{markdown}"))
    } else {
        tracing::warn!(
            "Content insufficient with reqwest (length: {}), will retry with browser: {}",
            markdown.len(),
            url
        );
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Content insufficient",
        )))
    }
}

fn fetch_with_browser(
    browser: &headless_chrome::Browser,
    url: &str,
) -> Result<String, Box<dyn std::error::Error + Send>> {
    tracing::info!("Fetching with browser: {}", url);

    let tab = browser.new_tab()?;

    tab.navigate_to(url)?;

    FlexibleWaiter::new(&tab)
        .with_timeout(Duration::from_secs(15))
        .wait_smart()?;

    let elem = tab.wait_for_element("body")?;

    let html = elem.get_content()?;

    let (markdown, _is_sufficient) = process_html(&html);

    let _ = tab.close(false);

    Ok(format!("<{url}>\n\n{markdown}"))
}

pub async fn fetch(urls: Vec<String>) -> Result<Vec<String>, Box<dyn std::error::Error + Send>> {
    // Process each URL sequentially to handle browser initialization properly
    let mut results = Vec::with_capacity(urls.len());
    let mut browser: Option<headless_chrome::Browser> = None;

    for url in urls {
        // Try reqwest first
        match fetch_with_reqwest(&url).await {
            Ok(content) => {
                results.push(content);
                continue;
            }
            Err(e) => {
                tracing::debug!("reqwest failed for {}: {}", url, e);
            }
        }

        // Initialize browser if not already done
        if browser.is_none() {
            tracing::info!("Initializing browser for fallback fetching");
            match headless_chrome::Browser::new(headless_chrome::LaunchOptions {
                headless: true,
                sandbox: false,
                devtools: false,
                enable_gpu: false,
                enable_logging: false,
                path: Some(PathBuf::from("/bin/chrome-headless-shell")),
                args: vec![
                    &std::ffi::OsString::from("--disable-setuid-sandbox"),
                    &std::ffi::OsString::from("--disable-dev-shm-usage"),
                    &std::ffi::OsString::from("--disable-software-rasterizer"),
                    &std::ffi::OsString::from("--single-process"),
                    &std::ffi::OsString::from("--no-zygote"),
                ],
                ..Default::default()
            }) {
                Ok(b) => browser = Some(b),
                Err(e) => {
                    tracing::error!("Failed to initialize browser: {}", e);
                    results.push(format!(
                        "Error fetching {}: Browser initialization failed: {}",
                        url, e
                    ));
                    continue;
                }
            }
        }

        // Fallback to browser
        if let Some(ref browser_instance) = browser {
            let url_clone = url.clone();
            let browser_clone = browser_instance.clone();

            match tokio::task::spawn_blocking(move || {
                fetch_with_browser(&browser_clone, &url_clone)
            })
            .await
            {
                Ok(Ok(content)) => results.push(content),
                Ok(Err(e)) => {
                    tracing::error!("Browser fetch failed for {}: {}", url, e);
                    results.push(format!("Error fetching {}: {}", url, e));
                }
                Err(e) => {
                    tracing::error!("Task spawn failed for {}: {}", url, e);
                    results.push(format!("Error spawning task for {}: {}", url, e));
                }
            }
        }
    }

    Ok(results)
}
