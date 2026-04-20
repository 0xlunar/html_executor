# html_executor
Execute Javascript from a given HTML page

## Example

```rust
use std::time::Duration;
use html_executor::{render_html, RenderOptions, DriverCapability};

#[tokio::main]
async fn main() {
    let response = reqwest::get("https://example.com/").await.unwrap();
    let url = response.url();
    let html = response.text().await.unwrap();

    let options = RenderOptions {
        html: Some(html.as_str()),
        url: url.as_str(),
        driver_url: None,
        output_delay: Some(Duration::from_secs(5)),
        driver_capability: DriverCapability::Chrome,
        user_agent: None,
        headless: true,
        cookie_only: false,
    };

    let rendered = render_html(options).await.unwrap();

    println!("{rendered:?}");
}
```