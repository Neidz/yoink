use std::collections::{HashSet, VecDeque};

use crate::url::Url;

pub struct Queue {
    pending: VecDeque<Url>,
    pending_set: HashSet<Url>,
    processing: HashSet<Url>,
    processed: HashSet<Url>,
    failed: HashSet<Url>,
}

impl Queue {
    pub fn new_with_initial(
        base_url: &Url,
        pending: Vec<Url>,
        processing: Vec<Url>,
        processed: Vec<Url>,
        failed: Vec<Url>,
    ) -> Self {
        let mut queue = Queue {
            pending: pending.clone().into_iter().collect(),
            pending_set: pending.iter().cloned().collect(),
            processing: processing.iter().cloned().collect(),
            processed: processed.iter().cloned().collect(),
            failed: failed.iter().cloned().collect(),
        };

        queue.add_pending(base_url);

        queue
    }

    pub fn add_pending(&mut self, url: &Url) {
        if !self.pending_set.contains(url)
            && !self.processed.contains(url)
            && !self.processing.contains(url)
        {
            self.pending.push_back(url.to_owned());
            self.pending_set.insert(url.to_owned());
        }
    }

    pub fn next(&mut self) -> Option<Url> {
        if let Some(url) = self.pending.pop_front() {
            self.pending_set.remove(&url);
            self.processing.insert(url.clone());

            return Some(url);
        }

        None
    }

    pub fn mark_as_processed(&mut self, url: &Url) {
        self.processing.remove(url);
        self.processed.insert(url.to_owned());
    }

    pub fn mark_as_failed(&mut self, url: &Url) {
        self.processing.remove(url);
        self.failed.insert(url.to_owned());
    }

    pub fn print_summary(&self) {
        println!(
            "Total: {}, pending: {}, processing: {}, processed: {}, failed: {}",
            self.pending_set.len()
                + self.processing.len()
                + self.processed.len()
                + self.failed.len(),
            self.pending.len(),
            self.processing.len(),
            self.processed.len(),
            self.failed.len()
        );
    }
}
