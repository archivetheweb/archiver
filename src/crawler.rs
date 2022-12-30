use std::{collections::HashSet, sync::Arc};

struct Crawler {
    visited: Arc<HashSet<String>>,
    depth: u8,
    url: String,
}

impl Crawler {
    fn new(url: String, depth: u8) -> Crawler {
        Crawler {
            visited: Arc::new(HashSet::new()),
            depth,
            url,
        }
    }
}
