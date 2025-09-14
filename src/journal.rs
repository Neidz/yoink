use std::{fs::File, path::PathBuf};

use tokio::{fs, sync::mpsc};

use crate::url::Url;

pub enum JournalEntry {
    Queued { url: Url },
    Started { url: Url },
    Finished { url: Url },
    Failed { url: Url, error: Option<String> },
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
