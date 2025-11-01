use std::{path::PathBuf, time::Duration};

use headless_chrome::Tab;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Input {
    pub url: String,
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

static BROWSER: tokio::sync::OnceCell<std::sync::Arc<headless_chrome::Browser>> =
    tokio::sync::OnceCell::const_new();

pub async fn fetch(url: &str) -> Result<String, Box<dyn std::error::Error + Send>> {
    let maybe_browser: Result<
        &std::sync::Arc<headless_chrome::Browser>,
        Box<dyn std::error::Error + Send>,
    > = BROWSER
        .get_or_try_init(|| async {
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

            Ok(std::sync::Arc::new(browser))
        })
        .await;

    let browser = maybe_browser?;

    let tab = browser.new_tab()?;

    tab.navigate_to(url)?;

    FlexibleWaiter::new(&tab)
        .with_timeout(Duration::from_secs(15))
        .wait_smart()?;

    let elem = tab.wait_for_element("body")?;

    let html = elem.get_content()?;

    let markdown = html2md::rewrite_html(&html, false);

    Ok(markdown)
}
