use std::{fmt, path::PathBuf, str::FromStr};

use tokio::{fs, sync::mpsc};

use crate::url::Url;

pub enum JournalEntry {
    Pending { url: Url },
    Processing { url: Url },
    Processed { url: Url },
    Failed { url: Url },
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
            "pending" => Ok(JournalEntry::Pending { url: url }),
            "processing" => Ok(JournalEntry::Processing { url: url }),
            "processed" => Ok(JournalEntry::Processed { url: url }),
            "failed" => Ok(JournalEntry::Failed { url: url }),
            _ => Err("invalid status".to_owned()),
        }
    }
}

pub struct Journal {
    sender: mpsc::UnboundedSender<JournalEntry>,
}

impl Journal {
    pub fn new(path: PathBuf) -> (Self, impl Future<Output = ()>) {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let task = async move {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .await
                .expect("failed to create journal file");
            while let Some(op) = rx.recv().await {}
        };

        (Journal { sender: tx }, task)
    }
}
