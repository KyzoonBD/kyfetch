//! kyfetch - simple internal-URL crawler (mini Screaming Frog).
//! Async BFS crawl, same-domain links only. Interactive or flag-driven.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use dialoguer::{Input, Select};
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use scraper::{Html, Selector};
use tokio::sync::Mutex;
use url::Url;

const USER_AGENT: &str = "kyfetch/1.0 internal-crawler";

#[derive(Parser)]
#[command(name = "kyfetch", version, about = "Fetch internal URLs of a site (mini Screaming Frog).")]
struct Args {
    /// Start URL, e.g. https://example.com. Omit to run interactive.
    url: Option<String>,

    /// Max pages to crawl (0 = all)
    #[arg(short = 'n', long, default_value_t = 500)]
    max_pages: usize,

    /// Concurrent requests
    #[arg(short = 'c', long, default_value_t = 20)]
    concurrency: usize,

    /// Request timeout (seconds)
    #[arg(short = 't', long, default_value_t = 10)]
    timeout: u64,

    /// Delay between requests in milliseconds (rate-limit, avoid blocks)
    #[arg(short = 'i', long, default_value_t = 0)]
    interval: u64,

    /// Write URLs to text file, one per line
    #[arg(short = 'o', long)]
    output: Option<String>,

    /// Export results to an .xlsx spreadsheet
    #[arg(short = 'x', long)]
    xlsx: Option<String>,
}

/// Resolved crawl settings (from flags or interactive prompts).
struct Config {
    start: Url,
    root_host: String,
    max_pages: usize,
    concurrency: usize,
    timeout: u64,
    interval: u64,
    interactive: bool,
    output: Option<String>,
    xlsx: Option<String>,
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

/// Parse a raw URL string, adding https:// if scheme missing.
fn parse_url(raw: &str) -> Result<Url, String> {
    let raw = raw.trim();
    let with_scheme = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    };
    Url::parse(&with_scheme).map_err(|e| e.to_string())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let cfg = match build_config(args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let results = crawl(&cfg).await;

    eprintln!("\nTotal: {} URLs", results.len());

    // Non-interactive file outputs (from flags).
    if let Some(path) = &cfg.output {
        save_txt(path, &results);
    }
    if let Some(path) = &cfg.xlsx {
        save_xlsx(path, &results);
    }

    // Interactive export prompt.
    if cfg.interactive {
        prompt_export(&results);
    }
}

/// Turn CLI args into a Config, prompting interactively when no URL given.
fn build_config(args: Args) -> Result<Config, String> {
    let interactive = args.url.is_none();

    let (start, max_pages, concurrency, timeout, interval) = if interactive {
        prompt_settings()?
    } else {
        let start = parse_url(args.url.as_ref().unwrap())?;
        let max = if args.max_pages == 0 { usize::MAX } else { args.max_pages };
        (start, max, args.concurrency, args.timeout, args.interval)
    };

    let start = normalize(start);
    let root_host = start.host_str().unwrap_or("").to_string();
    if root_host.is_empty() {
        return Err("URL has no host".into());
    }

    Ok(Config {
        start,
        root_host,
        max_pages,
        concurrency,
        timeout,
        interval,
        interactive,
        output: args.output,
        xlsx: args.xlsx,
    })
}

/// Interactive prompts for URL, page limit, and interval.
fn prompt_settings() -> Result<(Url, usize, usize, u64, u64), String> {
    println!("kyfetch — internal URL crawler\n");

    // URL (loop until parseable).
    let start = loop {
        let raw: String = Input::new()
            .with_prompt("Site URL")
            .interact_text()
            .map_err(|e| e.to_string())?;
        match parse_url(&raw) {
            Ok(u) => break u,
            Err(e) => eprintln!("  invalid URL: {e}\n"),
        }
    };

    // How many URLs, or all.
    let amount: String = Input::new()
        .with_prompt("How many URLs? (number or 'all')")
        .default("all".into())
        .interact_text()
        .map_err(|e| e.to_string())?;
    let max_pages = if amount.trim().eq_ignore_ascii_case("all") {
        usize::MAX
    } else {
        amount.trim().parse::<usize>().map_err(|_| "not a number".to_string())?
    };

    // Concurrency.
    let concurrency: usize = Input::new()
        .with_prompt("Concurrent requests")
        .default(20)
        .interact_text()
        .map_err(|e| e.to_string())?;

    // Interval between requests (safety / avoid blocks).
    let interval: u64 = Input::new()
        .with_prompt("Interval between requests in ms (0 = none)")
        .default(0)
        .interact_text()
        .map_err(|e| e.to_string())?;

    Ok((start, max_pages, concurrency.max(1), 10, interval))
}

/// Run the BFS crawl with a live progress bar.
async fn crawl(cfg: &Config) -> Vec<PageResult> {
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(cfg.timeout))
        .build()
        .expect("build client");

    let seen: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let queue: Arc<Mutex<Vec<Url>>> = Arc::new(Mutex::new(vec![cfg.start.clone()]));
    seen.lock().await.insert(cfg.start.as_str().to_string());

    let link_sel = Selector::parse("a[href]").unwrap();
    let mut results: Vec<PageResult> = Vec::new();

    // Progress spinner: shows fetched count + how many still queued.
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
    );
    pb.enable_steady_tick(Duration::from_millis(100));

    loop {
        if results.len() >= cfg.max_pages {
            break;
        }

        // Pull a batch from the queue.
        let batch: Vec<Url> = {
            let mut q = queue.lock().await;
            let remaining = cfg.max_pages.saturating_sub(results.len());
            let n = cfg.concurrency.min(remaining).min(q.len());
            q.drain(..n).collect()
        };
        if batch.is_empty() {
            break;
        }

        // Fire the batch. Stagger by `interval` to rate-limit.
        let mut futs = FuturesUnordered::new();
        for (idx, url) in batch.into_iter().enumerate() {
            let client = client.clone();
            let stagger = Duration::from_millis(cfg.interval * idx as u64);
            futs.push(async move {
                if !stagger.is_zero() {
                    tokio::time::sleep(stagger).await;
                }
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

                    if ctype.contains("text/html") {
                        if let Ok(body) = r.text().await {
                            let links = extract_links(&body, &final_url, &link_sel, &cfg.root_host);
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

            let queued = queue.lock().await.len();
            pb.set_message(format!("fetched {} · queued {}", results.len(), queued));
        }
    }

    pb.finish_and_clear();

    // Print result table.
    for r in &results {
        println!("{:>4}  {}  [{}]", r.status, r.url, r.ctype);
    }

    results
}

/// Ask the user how to export, then write the file(s).
fn prompt_export(results: &[PageResult]) {
    let choices = ["None", "Text (.txt)", "Excel (.xlsx)", "Both"];
    let pick = Select::new()
        .with_prompt("Export results?")
        .items(&choices)
        .default(0)
        .interact()
        .unwrap_or(0);

    let want_txt = pick == 1 || pick == 3;
    let want_xlsx = pick == 2 || pick == 3;

    if want_txt {
        let name: String = Input::new()
            .with_prompt("Text filename")
            .default("urls.txt".into())
            .interact_text()
            .unwrap_or_else(|_| "urls.txt".into());
        save_txt(&name, results);
    }
    if want_xlsx {
        let name: String = Input::new()
            .with_prompt("Excel filename")
            .default("urls.xlsx".into())
            .interact_text()
            .unwrap_or_else(|_| "urls.xlsx".into());
        save_xlsx(&name, results);
    }
}

/// Write URLs to a text file, one per line.
fn save_txt(path: &str, results: &[PageResult]) {
    let body: String = results.iter().map(|r| format!("{}\n", r.url)).collect();
    match std::fs::write(path, body) {
        Ok(()) => eprintln!("Saved {path}"),
        Err(e) => eprintln!("write failed: {e}"),
    }
}

/// Write results to xlsx, reporting the outcome.
fn save_xlsx(path: &str, results: &[PageResult]) {
    match write_xlsx(path, results) {
        Ok(()) => eprintln!("Exported {path}"),
        Err(e) => eprintln!("xlsx export failed: {e}"),
    }
}

/// Write results to an .xlsx spreadsheet: Status | URL | Content-Type.
fn write_xlsx(path: &str, results: &[PageResult]) -> Result<(), rust_xlsxwriter::XlsxError> {
    use rust_xlsxwriter::{Format, Workbook};

    let mut wb = Workbook::new();
    let sheet = wb.add_worksheet().set_name("URLs")?;

    let header = Format::new().set_bold();
    sheet.write_string_with_format(0, 0, "Status", &header)?;
    sheet.write_string_with_format(0, 1, "URL", &header)?;
    sheet.write_string_with_format(0, 2, "Content-Type", &header)?;

    for (i, r) in results.iter().enumerate() {
        let row = (i + 1) as u32;
        sheet.write_string(row, 0, &r.status)?;
        sheet.write_string(row, 1, &r.url)?;
        sheet.write_string(row, 2, &r.ctype)?;
    }

    sheet.set_column_width(0, 8)?;
    sheet.set_column_width(1, 70)?;
    sheet.set_column_width(2, 20)?;
    sheet.autofilter(0, 0, results.len() as u32, 2)?;

    wb.save(path)?;
    Ok(())
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
