use anyhow::format_err;
use std::time::Duration;
use thirtyfour::common::capabilities::firefox::FirefoxPreferences;
use thirtyfour::{
    CapabilitiesHelper, ChromiumLikeCapabilities, Cookie, PageLoadStrategy, WebDriver,
};
use tokio::time::Instant;
#[cfg(all(feature = "wreq", not(feature = "reqwest")))]
use wreq as reqwest;

static CHROMEDRIVER_CONNECTION_URL: &str = "http://127.0.0.1:4444";
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
    async fn render(
        self,
        chromedriver_url: Option<&str>,
        output_delay: Option<Duration>,
        driver_capability: DriverCapability,
        user_agent: Option<&str>,
        use_html: bool,
        headless: bool,
    ) -> anyhow::Result<RenderResults>;
}

#[async_trait::async_trait]
impl HTMLRendererExt for reqwest::Response {
    async fn render(
        self,
        chromedriver_url: Option<&str>,
        output_delay: Option<Duration>,
        driver_capability: DriverCapability,
        user_agent: Option<&str>,
        use_html: bool,
        headless: bool,
    ) -> anyhow::Result<RenderResults> {
        let url = self.url().to_string();

        let text = self.text().await?;
        let text = if use_html { Some(&*text) } else { None };

        let render_options = RenderOptions {
            html: text,
            url: url.as_str(),
            driver_url: chromedriver_url,
            output_delay,
            driver_capability,
            user_agent,
            headless,
            cookie_only: false,
        };

        let output = render_html(render_options).await?;
        Ok(output)
    }
}

#[derive(Default, Debug)]
pub struct RenderOptions<'a> {
    /// should be the whole html response from a request
    pub html: Option<&'a str>,
    /// should be at least the base url where the request is from,
    /// but depending on your use case could use any valid url.
    pub url: &'a str,
    /// should be the address and port where the chromedriver is running.
    /// Default will be used if not set.
    pub driver_url: Option<&'a str>,
    /// can be any duration, but it is recommended that a minimum of 2 seconds is used.
    /// Default will be used if not set.
    pub output_delay: Option<Duration>,
    /// the web driver being used (ie, Chrome/Firefox)
    /// Default is Chrome
    pub driver_capability: DriverCapability,
    /// Set a custom User-Agent
    /// Defaults to webdrivers settings
    pub user_agent: Option<&'a str>,
    /// Headless mode for webdriver if supported
    pub headless: bool,
    /// Only return cookies once finished
    pub cookie_only: bool,
}

#[derive(Debug)]
pub struct RenderResults {
    /// Final URL
    pub url: url::Url,
    /// Cookies from rendering
    pub cookies: Vec<Cookie>,
    /// Body from rendering if enabled
    pub body: Option<String>,
}

#[derive(Default, Debug)]
pub enum DriverCapability {
    #[default]
    Chrome,
    Firefox,
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
pub async fn render_html(options: RenderOptions<'_>) -> anyhow::Result<RenderResults> {
    let chromedriver_url = options.driver_url.unwrap_or(CHROMEDRIVER_CONNECTION_URL);
    let output_delay = options.output_delay.unwrap_or(Duration::from_secs(2));

    let browser = match options.driver_capability {
        DriverCapability::Chrome => {
            let mut caps = thirtyfour::DesiredCapabilities::chrome();

            if options.html.is_some() {
                caps.set_page_load_strategy(PageLoadStrategy::None)?;
            } else {
                caps.set_page_load_strategy(PageLoadStrategy::Normal)?;
            }
            if options.headless {
                caps.set_headless()?;
            }
            caps.set_no_sandbox()?;
            caps.add_arg("--disable-blink-features=AutomationControlled")?;
            caps.add_arg("--disable-extensions")?;
            caps.add_arg("--profile-directory=Default")?;

            if let Some(ua) = options.user_agent {
                let ua = format!("--user-agent={}", ua);
                caps.add_arg(&ua)?;
            }

            // caps.add_arg("--incognito")?;
            // caps.add_arg("--disable-plugins-discovery")?;
            caps.add_experimental_option("excludeSwitches", vec!["enable-automation"])?;
            WebDriver::new(chromedriver_url, caps).await?
        }
        DriverCapability::Firefox => {
            let mut caps = thirtyfour::DesiredCapabilities::firefox();
            caps.set_page_load_strategy(PageLoadStrategy::None)?;
            caps.set_headless()?;

            let mut prefs = FirefoxPreferences::new();
            if let Some(user_agent) = options.user_agent {
                prefs.set_user_agent(user_agent.to_string())?;
            }
            prefs.set("dom.webdriver.enabled", false)?;
            prefs.set("useAutomationExtension", false)?;

            caps.set_preferences(prefs)?;
            WebDriver::new(chromedriver_url, caps).await?
        }
    };

    // let browser = WebDriver::new(chromedriver_url, caps).await?;
    browser.goto(options.url).await?;
    // Only works if ran before the page has loaded, otherwise renderer freezes up.
    // Replaces entire content of page with our own HTML
    if let Some(html) = options.html {
        browser
            .execute(
                r#"
        document.write(arguments[0]);
    "#,
                vec![serde_json::to_value(html)?],
            )
            .await?;
    }

    tokio::time::sleep_until(Instant::now() + output_delay).await;

    // Check to ensure the renderer hasn't frozen up
    let mut interval = tokio::time::interval(Duration::from_secs(15));
    interval.tick().await; // Should immediately tick

    let html = if !options.cookie_only {
        if let Some(val) = tokio::select! {
            Ok(val) = browser.source() => {
                Some(val)
            }
            _ = interval.tick() => None
        } {
            Some(val)
        } else {
            return Err(format_err!("Page source retrieval timed out."));
        }
    } else {
        None
    };

    let url = browser.current_url().await?;
    let cookies = browser.get_all_cookies().await?;

    let results = RenderResults {
        url,
        cookies,
        body: html,
    };

    // Clean up session
    browser.quit().await?;

    Ok(results)
}
