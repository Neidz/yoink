use std::{
    collections::{HashSet, VecDeque},
    fs::create_dir_all,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::Result;
use clap::Parser;
use reqwest::Client;
use scraper::{Html, Selector};
use tokio::{
    fs::File,
    io::AsyncWriteExt,
    sync::{Mutex, Semaphore},
    task::JoinSet,
    time::interval,
};
use url::Url;
use urlencoding::encode;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    url: Url,
    #[arg(long, default_value_t = 100)]
    depth_limit: usize,
    #[arg(long, default_value_t = 100)]
    concurrency_limit: usize,
    #[arg(long, default_value_t = 1000)]
    request_timeout_ms: u64,
    #[arg(long, default_value_t = 100)]
    min_interval_ms: u64,
    #[arg(long, default_value = "Mozilla/5.0")]
    user_agent: String,
    #[arg(long, default_value = "scraper_output")]
    output_directory: PathBuf,
    #[arg(long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let html_directory = args.output_directory.join("html");
    create_dir_all(&html_directory).expect("failed to create output directory");
    let html_directory = Arc::new(html_directory);

    let client = Client::builder()
        .user_agent(args.user_agent)
        .timeout(Duration::from_millis(args.request_timeout_ms))
        .build()?;
    let base_path = args.url;
    let link_selector = Selector::parse("a").expect("failed to parse anchor tag selector");

    let queue = Arc::new(Mutex::new(Queue::new(base_path, args.depth_limit)));

    let semaphore = Arc::new(Semaphore::new(args.concurrency_limit));
    let mut join_set = JoinSet::new();
    let failed_counter = Arc::new(Mutex::new(0));
    let success_counter = Arc::new(Mutex::new(0));

    let delay = Duration::from_millis(args.min_interval_ms);
    let interval = Arc::new(Mutex::new(interval(delay)));

    loop {
        let next = {
            let mut queue = queue.lock().await;
            queue.next()
        };

        if let Some((url, current_depth)) = next {
            let permit = semaphore
                .clone()
                .acquire_owned()
                .await
                .expect("failed to acquire permit from semaphore");
            let queue = queue.clone();
            let client = client.clone();
            let link_selector = link_selector.clone();
            let html_directory = html_directory.clone();

            let failed_counter = failed_counter.clone();
            let success_counter = success_counter.clone();
            let interval = interval.clone();

            join_set.spawn(async move {
                let _permit = permit;

                {
                    let mut interval = interval.lock().await;
                    interval.tick().await;
                }

                let response = client.get(&url).send().await;
                match response {
                    Ok(resp) => {
                        if let Ok(body) = resp.text().await {
                            let urls = extract_links_from_body(&body, &link_selector);

                            let mut queue = queue.lock().await;
                            queue.add(&urls, current_depth + 1);
                            queue.done(&url);

                            if let Err(err) = save_html(&html_directory, &url, &body).await {
                                eprintln!("Failed to save html from {url}: {err}");

                                if args.verbose {
                                    let mut failed_counter = failed_counter.lock().await;
                                    *failed_counter += 1;
                                }
                            } else if args.verbose {
                                let mut success_counter = success_counter.lock().await;
                                *success_counter += 1;
                            }
                        } else {
                            eprintln!("Failed to read body from {url}");

                            if args.verbose {
                                let mut failed_counter = failed_counter.lock().await;
                                *failed_counter += 1;
                            }
                        }
                    }
                    Err(err) => eprintln!("Request failed: {err}"),
                }

                if args.verbose {
                    let success_counter = success_counter.lock().await;
                    let failed_counter = failed_counter.lock().await;
                    println!(
                        "Request count: {}, success: {}, failed: {}",
                        *success_counter + *failed_counter,
                        success_counter,
                        failed_counter
                    )
                }
            });
        } else {
            if join_set.is_empty() {
                break;
            }

            join_set.join_next().await;
        }
    }

    while let Some(res) = join_set.join_next().await {
        if let Err(err) = res {
            eprintln!("Task failed: {err:?}");
        }
    }

    if args.verbose {
        let queue = queue.lock().await;
        println!("Visited {} URLs", queue.visited.len());
    }

    Ok(())
}

type VisitDepth = usize;
type VisitUrl = String;

type QueueItem = (VisitUrl, VisitDepth);

struct Queue {
    depth_limit: usize,
    base_path: Url,
    to_visit: VecDeque<QueueItem>,
    visited: HashSet<VisitUrl>,
    visiting: HashSet<VisitUrl>,
}

impl Queue {
    fn new(base_path: Url, depth_limit: usize) -> Self {
        let mut queue = Queue {
            depth_limit,
            base_path: base_path.clone(),
            to_visit: VecDeque::new(),
            visited: HashSet::new(),
            visiting: HashSet::new(),
        };

        queue.add(&[base_path.to_string()], 0);

        queue
    }

    fn next(&mut self) -> Option<QueueItem> {
        if let Some((visit_url, visit_depth)) = self.to_visit.pop_front() {
            self.visiting.insert(visit_url.clone());

            return Some((visit_url, visit_depth));
        }

        None
    }

    fn add(&mut self, urls: &[String], depth: VisitDepth) {
        if depth > self.depth_limit {
            return;
        }

        for url in urls {
            if let Some(url) = normalize_and_filter_url(&self.base_path, url) {
                if !self.visited.contains(&url) && !self.visiting.contains(&url) {
                    self.to_visit.push_back((url.to_string(), depth));
                }
            }
        }
    }

    fn done(&mut self, url: &str) {
        self.visiting.remove(url);
        self.visited.insert(url.to_owned());
    }
}

fn normalize_and_filter_url(base: &Url, url_or_path: &str) -> Option<String> {
    let url = match Url::parse(url_or_path) {
        Ok(url) => Ok(url),
        Err(url::ParseError::RelativeUrlWithoutBase) => base.join(url_or_path),
        Err(err) => Err(err),
    };

    let mut url = match url {
        Ok(url) => url,
        Err(_) => return None,
    };

    if url.domain() != base.domain() || url.scheme() != base.scheme() {
        return None;
    }

    url.set_fragment(None);

    Some(url.to_string())
}

fn extract_links_from_body(body: &str, link_selector: &Selector) -> Vec<String> {
    let document = Html::parse_document(body);

    document
        .select(link_selector)
        .filter_map(|link| link.attr("href").map(String::from))
        .collect()
}

async fn save_html(html_directory: &Path, url: &str, html: &str) -> Result<()> {
    let encoded_url = encode(url);
    let file_path = html_directory.join(format!("{encoded_url}.html"));

    let mut file = File::create(file_path).await?;
    file.write_all(html.as_bytes()).await?;

    Ok(())
}
