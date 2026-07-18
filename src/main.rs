//! kyfetch - simple internal-URL crawler (mini Screaming Frog).
//! Async BFS crawl, same-domain links only.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use futures::stream::{FuturesUnordered, StreamExt};
use reqwest::Client;
use scraper::{Html, Selector};
use tokio::sync::Mutex;
use url::Url;

const USER_AGENT: &str = "kyfetch/1.0 internal-crawler";

#[derive(Parser)]
#[command(name = "kyfetch", about = "Fetch internal URLs of a site (mini Screaming Frog).")]
struct Args {
    /// Start URL, e.g. https://example.com
    url: String,

    /// Max pages to crawl
    #[arg(short = 'n', long, default_value_t = 500)]
    max_pages: usize,

    /// Concurrent requests
    #[arg(short = 'c', long, default_value_t = 20)]
    concurrency: usize,

    /// Request timeout (seconds)
    #[arg(short = 't', long, default_value_t = 10)]
    timeout: u64,

    /// Write URLs to file, one per line
    #[arg(short = 'o', long)]
    output: Option<String>,
}

/// A crawled page result.
struct PageResult {
    url: String,
    status: String,
    ctype: String,
}

/// Strip fragment + trailing slash so URLs dedupe cleanly.
fn normalize(mut u: Url) -> Url {
    u.set_fragment(None);
    if u.path().ends_with('/') && u.path().len() > 1 {
        let p = u.path().trim_end_matches('/').to_string();
        u.set_path(&p);
    }
    u
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Add scheme if missing.
    let start_raw = if args.url.contains("://") {
        args.url.clone()
    } else {
        format!("https://{}", args.url)
    };

    let start = match Url::parse(&start_raw) {
        Ok(u) => normalize(u),
        Err(e) => {
            eprintln!("bad URL: {e}");
            std::process::exit(1);
        }
    };
    let root_host = start.host_str().unwrap_or("").to_string();

    let client = Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(args.timeout))
        .build()
        .expect("build client");

    // Shared crawl state.
    let seen: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let queue: Arc<Mutex<Vec<Url>>> = Arc::new(Mutex::new(vec![start.clone()]));
    seen.lock().await.insert(start.as_str().to_string());

    let link_sel = Selector::parse("a[href]").unwrap();
    let mut results: Vec<PageResult> = Vec::new();

    // Drain queue in waves; each wave runs up to `concurrency` fetches.
    loop {
        if results.len() >= args.max_pages {
            break;
        }

        // Pull a batch from the queue.
        let batch: Vec<Url> = {
            let mut q = queue.lock().await;
            let take = args.concurrency.min(args.max_pages - results.len());
            let n = take.min(q.len());
            q.drain(..n).collect()
        };
        if batch.is_empty() {
            break;
        }

        let mut futs = FuturesUnordered::new();
        for url in batch {
            let client = client.clone();
            futs.push(async move {
                let resp = client.get(url.clone()).send().await;
                (url, resp)
            });
        }

        while let Some((url, resp)) = futs.next().await {
            match resp {
                Err(e) => {
                    let msg = if e.is_timeout() { "TIMEOUT".into() } else { format!("{e}") };
                    results.push(PageResult {
                        url: url.to_string(),
                        status: "ERR".into(),
                        ctype: msg.chars().take(50).collect(),
                    });
                }
                Ok(r) => {
                    let status = r.status().as_u16().to_string();
                    let ctype = r
                        .headers()
                        .get(reqwest::header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .split(';')
                        .next()
                        .unwrap_or("")
                        .to_string();
                    let final_url = r.url().clone();

                    // Only parse HTML for more links.
                    if ctype.contains("text/html") {
                        if let Ok(body) = r.text().await {
                            let links = extract_links(&body, &final_url, &link_sel, &root_host);
                            let mut seen_g = seen.lock().await;
                            let mut q = queue.lock().await;
                            for l in links {
                                if seen_g.insert(l.as_str().to_string()) {
                                    q.push(l);
                                }
                            }
                        }
                    }

                    results.push(PageResult { url: url.to_string(), status, ctype });
                }
            }
        }
    }

    // Print table.
    for r in &results {
        println!("{:>4}  {}  [{}]", r.status, r.url, r.ctype);
    }
    eprintln!("\nTotal: {} URLs", results.len());

    if let Some(path) = args.output {
        let body: String = results.iter().map(|r| format!("{}\n", r.url)).collect();
        if let Err(e) = std::fs::write(&path, body) {
            eprintln!("write failed: {e}");
        } else {
            eprintln!("Saved to {path}");
        }
    }
}

/// Parse HTML, return same-domain, normalized, http(s) links.
fn extract_links(body: &str, base: &Url, sel: &Selector, root_host: &str) -> Vec<Url> {
    let doc = Html::parse_document(body);
    let mut out = Vec::new();
    for el in doc.select(sel) {
        let Some(href) = el.value().attr("href") else { continue };
        let Ok(joined) = base.join(href) else { continue };
        if !matches!(joined.scheme(), "http" | "https") {
            continue;
        }
        if joined.host_str() != Some(root_host) {
            continue;
        }
        out.push(normalize(joined));
    }
    out
}
