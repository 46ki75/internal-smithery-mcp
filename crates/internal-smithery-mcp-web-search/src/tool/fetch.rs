use std::{path::PathBuf, time::Duration};

use headless_chrome::Tab;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Input {
    /// A list of URLs to fetch.
    pub urls: Vec<String>,
}

/// Process HTML to markdown
fn process_html(html: &str) -> String {
    html2md::rewrite_html(html, false)
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

            std::thread::sleep(Duration::from_millis(200));
        }

        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "Timeout: No suitable element found",
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

    let markdown = process_html(&html);

    let _ = tab.close(false);

    Ok(format!("<{url}>\n\n{markdown}"))
}

pub async fn fetch(urls: Vec<String>) -> Result<Vec<String>, Box<dyn std::error::Error + Send>> {
    // Process each URL sequentially to handle browser initialization properly
    let mut results = Vec::with_capacity(urls.len());

    // Initialize browser at the beginning
    tracing::info!("Initializing browser");
    let browser = headless_chrome::Browser::new(headless_chrome::LaunchOptions {
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
    })?;

    for url in urls {
        let url_clone = url.clone();
        let browser_clone = browser.clone();

        match tokio::task::spawn_blocking(move || fetch_with_browser(&browser_clone, &url_clone))
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

    Ok(results)
}
