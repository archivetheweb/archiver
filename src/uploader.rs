use std::fs;

use crate::utils::{ARCHIVE_DIR, BASE_DIR};

pub struct Uploader {}

impl Uploader {
    pub fn new() -> Self {
        Uploader {}
    }
    pub fn fetch_latest_warc(&self) -> anyhow::Result<()> {
        let dir = fs::read_dir(format!("./{}/{}/archive", BASE_DIR, ARCHIVE_DIR))?;

        let min = dir.into_iter().map(|x| x.unwrap()).max_by_key(|x| {
            let file = x.file_name();

            let elems: Vec<&str> = file.to_str().unwrap().trim().split("-").collect();

            println!("{elems:?}");

            elems[1].parse::<i64>().unwrap()
        });
        let m = min.unwrap();
        println!("{m:?}");
        // for file in min {
        //     println!("{file:?}")
        // }
        Ok(())
    }
}
