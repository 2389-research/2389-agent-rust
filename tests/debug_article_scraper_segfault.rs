//! Minimal test to reproduce article_scraper segfault
//!
//! This test is designed to isolate the segfault issue that occurs during
//! cleanup after tests complete on Linux but not macOS.
//!
//! Run with: cargo test --test debug_article_scraper_segfault -- --test-threads=1

use article_scraper::Readability;

#[tokio::test]
async fn test_minimal_article_extraction() {
    println!("START: test_minimal_article_extraction");

    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test Article</title></head>
        <body>
            <article>
                <h1>Main Article Title</h1>
                <p>This is the main content of the article.</p>
                <p>It should extract this text cleanly.</p>
            </article>
        </body>
        </html>
    "#;

    let result = Readability::extract(html, None).await;
    println!("Extraction result: {:?}", result.is_ok());

    assert!(result.is_ok());

    println!("END: test_minimal_article_extraction (before drop)");
    // Explicit drop to see if that's where it crashes
    drop(result);
    println!("END: test_minimal_article_extraction (after drop)");
}

#[tokio::test]
async fn test_multiple_extractions() {
    println!("START: test_multiple_extractions");

    let html = "<html><body><p>Test</p></body></html>";

    for i in 0..3 {
        println!("Iteration {}", i);
        let result = Readability::extract(html, None).await;
        assert!(result.is_ok() || result.is_err());
        drop(result);
        println!("Iteration {} dropped", i);
    }

    println!("END: test_multiple_extractions");
}

#[tokio::test]
async fn test_extraction_with_url() {
    println!("START: test_extraction_with_url");

    let html = "<html><body><p>Content</p></body></html>";
    let url = url::Url::parse("https://example.com/article").ok();

    let result = Readability::extract(html, url).await;
    println!("Extraction with URL result: {:?}", result.is_ok());

    assert!(result.is_ok() || result.is_err());

    println!("END: test_extraction_with_url");
}

#[test]
fn test_no_tokio() {
    println!("START: test_no_tokio (blocking)");

    // Test without tokio runtime to see if that matters
    let rt = tokio::runtime::Runtime::new().unwrap();
    let html = "<html><body><p>Test</p></body></html>";

    let result = rt.block_on(async {
        Readability::extract(html, None).await
    });

    println!("Blocking extraction result: {:?}", result.is_ok());
    assert!(result.is_ok() || result.is_err());

    drop(result);
    drop(rt);

    println!("END: test_no_tokio");
}

#[test]
fn test_sequential_runtimes() {
    println!("START: test_sequential_runtimes");

    let html = "<html><body><p>Test</p></body></html>";

    // Create and destroy multiple runtimes
    for i in 0..3 {
        println!("Runtime {}", i);
        let rt = tokio::runtime::Runtime::new().unwrap();

        let result = rt.block_on(async {
            Readability::extract(html, None).await
        });

        assert!(result.is_ok() || result.is_err());

        drop(result);
        println!("Result {} dropped", i);
        drop(rt);
        println!("Runtime {} dropped", i);
    }

    println!("END: test_sequential_runtimes");
}

// Test that explicitly forces garbage collection
#[tokio::test]
async fn test_with_forced_cleanup() {
    println!("START: test_with_forced_cleanup");

    let html = "<html><body><p>Test</p></body></html>";

    {
        let result = Readability::extract(html, None).await;
        assert!(result.is_ok() || result.is_err());
        // Explicit scope to force drop
    }

    println!("After scope drop");

    // Force some memory pressure
    let _temp: Vec<u8> = vec![0; 1024 * 1024];

    println!("END: test_with_forced_cleanup");
}
