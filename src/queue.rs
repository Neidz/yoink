use std::collections::{HashSet, VecDeque};

use crate::url::Url;

pub struct Queue {
    pending: VecDeque<Url>,
    pending_set: HashSet<Url>,
    processing: HashSet<Url>,
    processed: HashSet<Url>,
}

impl Queue {
    pub fn new(base_url: &Url) -> Self {
        let mut queue = Queue {
            pending: VecDeque::new(),
            pending_set: HashSet::new(),
            processing: HashSet::new(),
            processed: HashSet::new(),
        };

        queue.add(base_url);

        queue
    }

    pub fn add(&mut self, url: &Url) {
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

    pub fn done(&mut self, url: &Url) {
        self.processing.remove(url);
        self.processed.insert(url.to_owned());
    }

    pub fn processed_amount(&self) -> usize {
        self.processed.len()
    }
}
