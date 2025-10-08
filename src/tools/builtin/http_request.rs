//! HTTP request tool implementation
//!
//! This module implements the HTTP request builtin tool for making HTTP requests
//! with optional content extraction for HTML responses using article_scraper.

use crate::tools::{Tool, ToolDescription, ToolError};
use article_scraper::Readability;
use async_trait::async_trait;
use serde_json::{json, Value};
use url::Url;

/// HTTP request tool - builtin implementation
pub struct HttpRequestTool {
    client: Option<reqwest::Client>,
    max_response_size: usize,
}

impl Default for HttpRequestTool {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpRequestTool {
    pub fn new() -> Self {
        Self {
            client: None,
            max_response_size: 1024 * 1024, // 1MB default
        }
    }

    /// Extract readable content from HTML using article_scraper (Mozilla Readability)
    async fn extract_readable_content(&self, html: &str, url: &str) -> Result<String, String> {
        // Parse URL
        let parsed_url = match Url::parse(url) {
            Ok(u) => Some(u),
            Err(_e) => {
                tracing::debug!("Failed to parse URL '{url}', using simple extraction");
                return Ok(self.simple_html_to_text(html));
            }
        };

        // Catch panics from article_scraper library - it can panic on malformed HTML
        // We suppress article_scraper's debug logs by running in spawn_blocking
        let html_owned = html.to_string();
        let url_owned = url.to_string();
        let result = tokio::task::spawn_blocking(move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // Run the extraction in a blocking context since we need catch_unwind
                tokio::runtime::Handle::current()
                    .block_on(async { Readability::extract(&html_owned, parsed_url.clone()).await })
            }))
        })
        .await;

        match result {
            Ok(Ok(Ok(article_text))) => Ok(article_text),
            Ok(Ok(Err(e))) => {
                tracing::debug!(
                    "Article extraction failed: {}, falling back to simple extraction",
                    e
                );
                Ok(self.simple_html_to_text(html))
            }
            Ok(Err(_panic)) => {
                // article_scraper panicked - return error to be reported
                Err(format!(
                    "Content extraction failed for URL '{url_owned}': HTML parsing library encountered an error with this page's structure"
                ))
            }
            Err(e) => {
                tracing::warn!("Article extraction task failed: {}", e);
                Ok(self.simple_html_to_text(html))
            }
        }
    }

    /// Simple HTML to text conversion fallback (pure function)
    fn simple_html_to_text(&self, html: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;
        let mut in_script = false;
        let mut in_style = false;
        let mut tag_name = String::new();
        let chars = html.chars();

        for ch in chars {
            match ch {
                '<' => {
                    in_tag = true;
                    tag_name.clear();
                }
                '>' => {
                    if in_tag {
                        let tag_lower = tag_name.to_lowercase();

                        // Handle script and style tags
                        match tag_lower.as_str() {
                            "script" => in_script = true,
                            "/script" => in_script = false,
                            "style" => in_style = true,
                            "/style" => in_style = false,
                            _ => {}
                        }

                        // Add spacing for block elements
                        if matches!(
                            tag_lower.as_str(),
                            "div"
                                | "p"
                                | "br"
                                | "h1"
                                | "h2"
                                | "h3"
                                | "h4"
                                | "h5"
                                | "h6"
                                | "li"
                                | "/div"
                                | "/p"
                                | "/h1"
                                | "/h2"
                                | "/h3"
                                | "/h4"
                                | "/h5"
                                | "/h6"
                                | "/li"
                        ) {
                            result.push('\n');
                        }

                        in_tag = false;
                        tag_name.clear();
                    }
                }
                _ => {
                    if in_tag {
                        tag_name.push(ch);
                    } else if !in_script && !in_style {
                        // Only add content if not in script or style tags
                        if ch.is_whitespace() {
                            // Normalize whitespace
                            if !result.ends_with(' ') && !result.ends_with('\n') {
                                result.push(' ');
                            }
                        } else {
                            result.push(ch);
                        }
                    }
                }
            }
        }

        // Clean up the result - compose multiple operations
        result
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string()
    }

    /// Build HTTP request from parameters (pure function)
    fn build_request_config(
        client: &reqwest::Client,
        method: &str,
        url: &str,
        headers: Option<&serde_json::Map<String, Value>>,
        body: Option<&str>,
        timeout: u64,
    ) -> Result<reqwest::RequestBuilder, ToolError> {
        let mut request = match method {
            "GET" => client.get(url),
            "POST" => client.post(url),
            "PUT" => client.put(url),
            "DELETE" => client.delete(url),
            "HEAD" => client.head(url),
            "OPTIONS" => client.request(reqwest::Method::OPTIONS, url),
            _ => return Err(ToolError::ExecutionError("Invalid HTTP method".to_string())),
        };

        if let Some(headers) = headers {
            for (key, value) in headers {
                if let Some(value_str) = value.as_str() {
                    request = request.header(key, value_str);
                }
            }
        }

        if let Some(body) = body {
            request = request.body(body.to_string());
        }

        request = request.timeout(std::time::Duration::from_secs(timeout));

        Ok(request)
    }

    /// Parse response into result JSON (pure function)
    fn format_response(
        status: u16,
        headers: std::collections::HashMap<String, String>,
        body: String,
        extract_content: bool,
        extracted_body: String,
    ) -> Value {
        json!({
            "status": status,
            "headers": headers,
            "body": if extract_content { extracted_body } else { body },
            "content_extracted": extract_content
        })
    }
}

#[async_trait]
impl Tool for HttpRequestTool {
    fn describe(&self) -> ToolDescription {
        ToolDescription {
            name: "http_request".to_string(),
            description: "Make HTTP requests with optional article content extraction using Mozilla Readability algorithm".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "method": {
                        "type": "string",
                        "enum": ["GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS"]
                    },
                    "url": {
                        "type": "string",
                        "format": "uri"
                    },
                    "extract_content": {
                        "type": "boolean",
                        "description": "Extract clean article content from HTML using Mozilla Readability (removes ads, navigation, etc). Highly recommended for research tasks.",
                        "default": false
                    },
                    "headers": {
                        "type": "object",
                        "additionalProperties": {
                            "type": "string"
                        }
                    },
                    "body": {
                        "type": "string"
                    },
                    "timeout": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 300
                    }
                },
                "required": ["method", "url"],
                "additionalProperties": false
            }),
        }
    }

    async fn initialize(&mut self, config: Option<&Value>) -> Result<(), ToolError> {
        if let Some(config) = config {
            if let Some(max_size) = config.get("max_response_size").and_then(|v| v.as_u64()) {
                self.max_response_size = max_size as usize;
            }
        }

        self.client = Some(
            reqwest::Client::builder()
                .build()
                .map_err(|e| ToolError::InitializationError(e.to_string()))?,
        );

        Ok(())
    }

    async fn execute(&self, parameters: &Value) -> Result<Value, ToolError> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionError("Tool not initialized".to_string()))?;

        // Extract parameters (pure parsing)
        let method = parameters["method"].as_str().unwrap();
        let url = parameters["url"].as_str().unwrap();
        let extract_content = parameters
            .get("extract_content")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let headers = parameters.get("headers").and_then(|h| h.as_object());
        let body = parameters.get("body").and_then(|b| b.as_str());
        let timeout = parameters
            .get("timeout")
            .and_then(|t| t.as_u64())
            .unwrap_or(30);

        // Build request using pure function
        let request = Self::build_request_config(client, method, url, headers, body, timeout)?;

        // Execute HTTP request (impure I/O)
        let response = request
            .send()
            .await
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        let status = response.status().as_u16();
        let response_headers: std::collections::HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let body = response
            .text()
            .await
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        if body.len() > self.max_response_size {
            return Err(ToolError::ExecutionError(format!(
                "Response too large: {} bytes (max: {})",
                body.len(),
                self.max_response_size
            )));
        }

        // Extract readable content if requested (async operation)
        let extracted_body = if extract_content {
            self.extract_readable_content(&body, url)
                .await
                .map_err(ToolError::ExecutionError)?
        } else {
            body.clone()
        };

        // Format response using pure function
        Ok(Self::format_response(
            status,
            response_headers,
            body,
            extract_content,
            extracted_body,
        ))
    }

    async fn shutdown(&mut self) -> Result<(), ToolError> {
        self.client = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_request_tool_creation() {
        let tool = HttpRequestTool::new();
        assert!(tool.client.is_none());
        assert_eq!(tool.max_response_size, 1024 * 1024);
    }

    #[test]
    fn test_simple_html_to_text() {
        let tool = HttpRequestTool::new();
        let html = "<html><body><h1>Title</h1><p>Paragraph</p><script>code</script></body></html>";
        let text = tool.simple_html_to_text(html);

        // Should extract text content and ignore script tags
        assert!(text.contains("Title"));
        assert!(text.contains("Paragraph"));
        assert!(!text.contains("code"));
        assert!(!text.contains("<h1>"));
    }

    #[test]
    fn test_tool_description() {
        let tool = HttpRequestTool::new();
        let description = tool.describe();

        assert_eq!(description.name, "http_request");
        assert!(!description.description.is_empty());
        assert!(description.parameters.is_object());
    }

    #[tokio::test]
    async fn test_article_extraction_with_valid_html() {
        let tool = HttpRequestTool::new();
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test Article</title></head>
            <body>
                <header>Navigation stuff</header>
                <article>
                    <h1>Main Article Title</h1>
                    <p>This is the main content of the article.</p>
                    <p>It should extract this text cleanly.</p>
                </article>
                <aside>Advertisement</aside>
                <footer>Footer content</footer>
            </body>
            </html>
        "#;

        let result = tool
            .extract_readable_content(html, "https://example.com/article")
            .await
            .expect("Extraction should succeed");

        // Should extract main content and ignore nav/ads/footer
        assert!(result.contains("main content"));
        assert!(!result.contains("Navigation stuff"));
        assert!(!result.contains("Advertisement"));
    }

    #[tokio::test]
    async fn test_article_extraction_with_invalid_url() {
        let tool = HttpRequestTool::new();
        let html = "<html><body><p>Content</p></body></html>";

        // Should fall back to simple extraction on invalid URL
        let result = tool
            .extract_readable_content(html, "not-a-valid-url")
            .await
            .expect("Simple extraction should succeed");

        assert!(result.contains("Content"));
    }

    #[tokio::test]
    async fn test_article_extraction_fallback() {
        let tool = HttpRequestTool::new();
        // Minimal HTML that might fail article extraction
        let html = "<html><body><div>Just a div</div></body></html>";

        let result = tool
            .extract_readable_content(html, "https://example.com")
            .await
            .expect("Extraction or fallback should succeed");

        // Should either extract or fall back to simple text extraction
        assert!(result.contains("Just a div") || result.contains("div"));
    }

    #[test]
    fn test_simple_html_to_text_removes_scripts_and_styles() {
        let tool = HttpRequestTool::new();
        let html = r#"
            <html>
            <head>
                <style>body { color: red; }</style>
            </head>
            <body>
                <h1>Title</h1>
                <p>Content</p>
                <script>console.log("test");</script>
            </body>
            </html>
        "#;

        let result = tool.simple_html_to_text(html);

        assert!(result.contains("Title"));
        assert!(result.contains("Content"));
        assert!(!result.contains("color: red"));
        assert!(!result.contains("console.log"));
    }
}
