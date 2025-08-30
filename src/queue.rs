use std::collections::{HashSet, VecDeque};

use crate::url::Url;

pub struct Queue {
    to_visit: VecDeque<Url>,
    visited: HashSet<Url>,
    visiting: HashSet<Url>,
}

impl Queue {
    pub fn new(base_url: &Url) -> Self {
        let mut queue = Queue {
            to_visit: VecDeque::new(),
            visited: HashSet::new(),
            visiting: HashSet::new(),
        };

        queue.add(base_url);

        queue
    }

    pub fn add(&mut self, url: &Url) {
        if !self.visited.contains(url) && !self.visiting.contains(url) {
            self.to_visit.push_back(url.to_owned());
        }
    }

    pub fn next(&mut self) -> Option<Url> {
        if let Some(url) = self.to_visit.pop_front() {
            self.visiting.insert(url.clone());

            return Some(url);
        }

        None
    }

    pub fn done(&mut self, url: &Url) {
        self.visiting.remove(url);
        self.visited.insert(url.to_owned());
    }

    pub fn visited_amount(&self) -> usize {
        self.visited.len()
    }
}
