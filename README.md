# html_executor
Execute Javascript from a given HTML page

## Supported request libraries
- [reqwest](https://crates.io/crates/reqwest)
- [rquest](https://crates.io/crates/rquest)

## Examples
**Reqwest**
```rust
use html_executor::HTMLRendererExt;

#[tokio::main]
async fn main() {
    let response = reqwest::get("https://example.com/").await.unwrap();
    let rendered = response.render(None, None).await.unwrap();
    
    println!("{rendered}");
}
```
**Rquest**
```rust
use html_executor::HTMLRendererExt;

#[tokio::main]
async fn main() {
    let response = rquest::get("https://example.com/").await.unwrap();
    let rendered = response.render(None, None).await.unwrap();
    
    println!("{rendered}");
}
```

**Non-Request Format**
```rust
use html_executor::{render_html, RenderOptions};

#[tokio::main]
async fn main() {
    let response = reqwest::get("https://example.com/").await.unwrap();
    let url = response.url();
    let html = response.text().await.unwrap();
    
    let options = RenderOptions {
        html: html.as_str(),
        url: url.as_str(),
        chromedriver_url: None,
        output_delay: None,
    };
    
    let rendered = render_html(options).await.unwrap();
    
    println!("{rendered}");
}
```