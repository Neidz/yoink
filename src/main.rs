use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

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

use crate::{
    encoding::url_encode,
    journal::{Journal, JournalEntry},
    queue::Queue,
};

mod encoding;
mod journal;
mod queue;
mod url;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    url: Url,
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
async fn main() {
    let args = Args::parse();

    let html_directory = args.output_directory.join("html");
    let journal_path = args.output_directory.join("journal.log");
    create_dir_all(&html_directory).expect("Failed to create output directory");
    let html_directory = Arc::new(html_directory);

    let client = Client::builder()
        .user_agent(args.user_agent)
        .timeout(Duration::from_millis(args.request_timeout_ms))
        .build()
        .expect("Failed to build client");
    let base_url = args.url;
    let link_selector = Selector::parse("a").expect("Failed to parse anchor tag selector");

    let jorunal_history = Journal::load_history(journal_path.clone());
    let queue = Arc::new(Mutex::new(Queue::new_with_initial(
        &base_url,
        jorunal_history.pending,
        jorunal_history.processing,
        jorunal_history.processed,
        jorunal_history.failed,
    )));
    let (journal, journal_task) = Journal::new(journal_path);
    let journal_handle = tokio::spawn(journal_task);

    let semaphore = Arc::new(Semaphore::new(args.concurrency_limit));
    let mut join_set = JoinSet::new();

    let delay = Duration::from_millis(args.min_interval_ms);
    let interval = Arc::new(Mutex::new(interval(delay)));

    loop {
        let next = {
            let mut queue = queue.lock().await;
            queue.next()
        };

        if let Some(url) = next {
            let permit = semaphore
                .clone()
                .acquire_owned()
                .await
                .expect("Failed to acquire permit from semaphore");
            let queue = queue.clone();
            let mut journal = journal.clone();
            let client = client.clone();
            let base_url = base_url.clone();
            let link_selector = link_selector.clone();
            let html_directory = html_directory.clone();

            let interval = interval.clone();

            journal.send(JournalEntry::Processing {
                url: url.to_owned(),
            });

            join_set.spawn(async move {
                let _permit = permit;

                {
                    let mut interval = interval.lock().await;
                    interval.tick().await;
                }

                let resp = match client.get(url.to_string()).send().await {
                    Ok(r) => r,
                    Err(err) => {
                        let mut queue = queue.lock().await;
                        queue.mark_as_failed(&url);
                        journal.send(JournalEntry::Failed {
                            url: url.to_owned(),
                        });
                        eprintln!("Request failed for {url}: {err}");
                        return;
                    }
                };
                let mut queue = queue.lock().await;

                let body = match resp.text().await {
                    Ok(b) => b,
                    Err(err) => {
                        queue.mark_as_failed(&url);
                        journal.send(JournalEntry::Failed {
                            url: url.to_owned(),
                        });
                        eprintln!("Failed to read body for {url}: {err}");
                        return;
                    }
                };

                let urls = extract_links_from_body(&body, &link_selector);

                for url_or_path in urls {
                    if let Ok(url) = Url::new_with_base(&base_url, &url_or_path) {
                        queue.add_pending(&url);
                        journal.send(JournalEntry::Pending {
                            url: url.to_owned(),
                        });
                    }
                }

                if let Err(err) = save_html(&html_directory, &url, &body).await {
                    queue.mark_as_failed(&url);
                    journal.send(JournalEntry::Failed {
                        url: url.to_owned(),
                    });
                    println!("Failed to save html for {url}: {err}")
                }

                queue.mark_as_processed(&url);
                journal.send(JournalEntry::Processed {
                    url: url.to_owned(),
                });

                if args.verbose {
                    queue.print_summary();
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
            eprintln!("Crawl task failed: {err:?}");
        }
    }

    drop(journal);
    if let Err(err) = journal_handle.await {
        eprintln!("Jornal task failed: {err}");
    }
}

fn extract_links_from_body(body: &str, link_selector: &Selector) -> Vec<String> {
    let document = Html::parse_document(body);

    document
        .select(link_selector)
        .filter_map(|link| link.attr("href").map(String::from))
        .collect()
}

async fn save_html(html_directory: &Path, url: &Url, html: &str) -> Result<(), String> {
    let encoded_url = url_encode(&url.to_string());
    let file_path = html_directory.join(format!("{encoded_url}.html"));

    let mut file = File::create(file_path)
        .await
        .map_err(|err| err.to_string())?;
    file.write_all(html.as_bytes())
        .await
        .map_err(|err| err.to_string())?;

    Ok(())
}
