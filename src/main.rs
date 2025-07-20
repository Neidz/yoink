use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};

use anyhow::Result;
use clap::Parser;
use reqwest::Client;
use scraper::{Html, Selector};
use tokio::{
    sync::{Mutex, Semaphore},
    task::JoinSet,
};
use url::Url;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    url: Url,
    #[arg(short, long, default_value_t = 1)]
    depth_limit: usize,
    #[arg(short, long, default_value_t = 100)]
    concurrency_limit: usize,
    #[arg(short, long, default_value = "Mozilla/5.0")]
    user_agent: String,
    #[arg(short, long, default_value_t = true)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let client = Client::builder().user_agent(args.user_agent).build()?;
    let base_path = args.url;
    let link_selector = Selector::parse("a").expect("failed to parse anchor tag selector");

    let queue = Arc::new(Mutex::new(Queue::new(base_path, args.depth_limit)));

    let semaphore = Arc::new(Semaphore::new(args.concurrency_limit));
    let mut join_set = JoinSet::new();

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

            join_set.spawn(async move {
                let _permit = permit;

                let response = client.get(&url).send().await;
                if let Ok(resp) = response {
                    if let Ok(body) = resp.text().await {
                        let urls = extract_links_from_body(&body, &link_selector);

                        let mut queue = queue.lock().await;
                        queue.add(&urls, current_depth + 1);
                        queue.done(&url);
                    } else {
                        eprintln!("Failed to read body from {url}");
                    }
                } else {
                    eprintln!("Request failed for {url}");
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
