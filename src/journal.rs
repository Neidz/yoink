use std::{collections::HashSet, fmt, io::BufRead, path::PathBuf, str::FromStr};

use tokio::{fs, io::AsyncWriteExt, sync::mpsc};

use crate::url::Url;

pub enum JournalEntry {
    Pending { url: Url },
    Processing { url: Url },
    Processed { url: Url },
    Failed { url: Url },
}

#[derive(Default)]
pub struct JournalHistory {
    pub pending: Vec<Url>,
    pub processing: Vec<Url>,
    pub processed: Vec<Url>,
    pub failed: Vec<Url>,
}

impl fmt::Display for JournalEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JournalEntry::Pending { url } => write!(f, "pending;{url}"),
            JournalEntry::Processing { url } => write!(f, "processing;{url}"),
            JournalEntry::Processed { url } => write!(f, "processed;{url}"),
            JournalEntry::Failed { url } => write!(f, "failed;{url}"),
        }
    }
}

impl FromStr for JournalEntry {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (status, url) = s.split_once(';').ok_or("invalid entry".to_owned())?;
        let url = Url::from_str(url).map_err(|err| err.to_string())?;

        match status {
            "pending" => Ok(JournalEntry::Pending { url }),
            "processing" => Ok(JournalEntry::Processing { url }),
            "processed" => Ok(JournalEntry::Processed { url }),
            "failed" => Ok(JournalEntry::Failed { url }),
            _ => Err("invalid status".to_owned()),
        }
    }
}

#[derive(Clone)]
pub struct Journal {
    sender: mpsc::UnboundedSender<JournalEntry>,
}

impl Journal {
    pub fn new(path: PathBuf) -> (Self, impl Future<Output = ()>) {
        let (tx, mut rx) = mpsc::unbounded_channel::<JournalEntry>();

        let task = async move {
            let mut f = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .await
                .expect("Failed to create journal file");

            while let Some(entry) = rx.recv().await {
                let line = format!("{entry}\n");
                if let Err(err) = f.write_all(line.as_bytes()).await {
                    eprintln!("Failed to write journal entry to the file: {err}");
                }
            }

            if let Err(err) = f.flush().await {
                eprintln!("Failed to flush the journal: {err}");
            }
        };

        (Journal { sender: tx }, task)
    }

    pub fn send(&mut self, entry: JournalEntry) {
        if let Err(err) = self.sender.send(entry) {
            eprintln!("Failed to send journal entry: {err}");
        }
    }

    pub fn load_history(path: PathBuf) -> JournalHistory {
        let f = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return JournalHistory::default();
            }
            Err(err) => {
                panic!("failed to read journal file {err}")
            }
        };
        let reader = std::io::BufReader::new(f);

        let mut maybe_pending = Vec::new();
        let mut maybe_processing = HashSet::new();
        let mut processed = HashSet::new();
        let mut failed = HashSet::new();

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(err) => {
                    eprintln!("failed to read journal line: {err}");
                    continue;
                }
            };

            let entry = match JournalEntry::from_str(&line) {
                Ok(entry) => entry,
                Err(err) => {
                    eprintln!("failed to deserialize journal line: {err}");
                    continue;
                }
            };

            match entry {
                JournalEntry::Pending { url } => {
                    maybe_pending.push(url);
                }
                JournalEntry::Processing { url } => {
                    maybe_processing.insert(url);
                }
                JournalEntry::Processed { url } => {
                    processed.insert(url);
                }
                JournalEntry::Failed { url } => {
                    failed.insert(url);
                }
            }
        }

        let pending: Vec<_> = maybe_pending
            .into_iter()
            .filter(|entry| {
                !maybe_processing.contains(entry)
                    && !processed.contains(entry)
                    && !failed.contains(entry)
            })
            .collect();
        let processing: Vec<_> = maybe_processing
            .into_iter()
            .filter(|entry| !processed.contains(entry) && !failed.contains(entry))
            .collect();
        let processed: Vec<_> = processed.into_iter().collect();
        let failed: Vec<_> = failed.into_iter().collect();

        JournalHistory {
            pending,
            processing,
            processed,
            failed,
        }
    }
}
