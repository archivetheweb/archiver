use std::fs::{self, DirEntry};

use crate::utils::{ARCHIVE_DIR, BASE_DIR};

pub struct Uploader {}

impl Uploader {
    pub fn new() -> Self {
        Uploader {}
    }
    pub fn fetch_latest_warc(&self) -> anyhow::Result<DirEntry> {
        let dir = fs::read_dir(format!("./{}/{}/archive", BASE_DIR, ARCHIVE_DIR))?;

        let latest = dir.into_iter().map(|x| x.unwrap()).max_by_key(|x| {
            let file = x.file_name();

            let elems: Vec<&str> = file.to_str().unwrap().trim().split("-").collect();

            elems[1].parse::<u128>().unwrap()
        });

        Ok(latest.unwrap())
    }
}
