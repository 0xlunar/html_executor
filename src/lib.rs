use anyhow::format_err;
#[cfg(feature = "reqwest")]
use reqwest;
#[cfg(all(feature = "rquest", not(feature = "reqwest")))]
use rquest as reqwest;
use std::time::Duration;
use thirtyfour::{CapabilitiesHelper, ChromiumLikeCapabilities, PageLoadStrategy, WebDriver};
use tokio::time::Instant;

static CHROMEDRIVER_CONNECTION_URL: &'static str = "http://127.0.0.1:4444";
#[async_trait::async_trait]
pub trait HTMLRendererExt {
    /// Render HTML from a response
    ///
    /// **Assumption**
    /// - Using chromedriver
    ///
    /// **No Guarantees**
    /// - Page may not finish rendering before output is given.
    ///
    /// **Defaults**
    /// - `chromedriver_url` defaults to `http://127.0.0.1:4444`
    /// - `output_delay` defaults to 2-Seconds
    async fn render(self, chromedriver_url: Option<&str>, output_delay: Option<Duration>) -> anyhow::Result<String>;
}

#[async_trait::async_trait]
impl HTMLRendererExt for reqwest::Response {
    async fn render(self, chromedriver_url: Option<&str>, output_delay: Option<Duration>) -> anyhow::Result<String> {
        let url = match self.url().host_str() {
            Some(host) => {
                format!("{}://{host}", self.url().scheme())
            },
            None => return Err(format_err!("Cannot-be-a-base URL not supported"))
        };

        let text = self.text().await?;

        let render_options = RenderOptions {
            html: text.as_str(),
            url: url.as_str(),
            chromedriver_url,
            output_delay
        };

        let output = render_html(render_options).await?;
        Ok(output)
    }
}

#[derive(Default, Debug)]
pub struct RenderOptions<'a> {
    /// should be the whole html response from a request
    pub html: &'a str,
    /// should be at least the base url where the request is from,
    /// but depending on your use case could use any valid url.
    pub url: &'a str,
    // user_agent: Option<&'a str>,
    /// should be the address and port where the chromedriver is running.
    /// Default will be used if not set.
    pub chromedriver_url: Option<&'a str>,
    /// can be any duration, but it is recommended that a minimum of 2 seconds is used.
    /// Default will be used if not set.
    pub output_delay: Option<Duration>,
}

/// Render HTML from a response
///
/// **Assumption**
/// - Using chromedriver
///
/// **No Guarantees**
/// - Page may not finish rendering before output is given.
///
/// **Defaults**
/// - `options.chromedriver_url` defaults to `http://127.0.0.1:4444`
/// - `options.output_delay` defaults to 2-Seconds
pub async fn render_html(options: RenderOptions<'_>) -> anyhow::Result<String> {
    let chromedriver_url = options.chromedriver_url.unwrap_or(CHROMEDRIVER_CONNECTION_URL);
    let output_delay = options.output_delay.unwrap_or(Duration::from_secs(2));

    let mut caps = thirtyfour::DesiredCapabilities::chrome();
    caps.set_page_load_strategy(PageLoadStrategy::None)?;
    caps.set_headless()?;

    let browser = WebDriver::new(chromedriver_url, caps).await?;
    browser.goto(options.url).await?;
    // Only works if ran before the page has loaded, otherwise renderer freezes up.
    // Replaces entire content of page with our own HTML
    browser.execute(r#"
        document.write(arguments[0]);
    "#, vec![serde_json::to_value(options.html)?]).await?;

    tokio::time::sleep_until(Instant::now() + output_delay).await;

    // Check to ensure the renderer hasn't frozen up
    let mut interval = tokio::time::interval(Duration::from_secs(15));
    interval.tick().await; // Should immediately tick
    let html = if let Some(val) = tokio::select! {
        Ok(val) = browser.source() => {
            Some(val)
        }
        _ = interval.tick() => None
    } {
        val
    } else {
        return Err(format_err!("Page source retrieval timed out."))
    };

    // Clean up session
    browser.quit().await?;

    Ok(html)
}
