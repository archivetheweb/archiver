use std::{
    fs::{self, DirEntry},
    path::PathBuf,
    str::FromStr,
};

use arloader::{
    transaction::{Base64, FromUtf8Strs},
    Arweave,
};
use bundlr_sdk::{
    currency::{arweave::Arweave as Ar, Currency},
    tags::Tag,
    ArweaveSigner, Bundlr,
};
use reqwest::Url;

use crate::utils::{ARCHIVE_DIR, BASE_DIR};

pub struct Uploader {
    // arweave: Arweave,
}

impl Uploader {
    pub async fn new(path: &str) -> anyhow::Result<Self> {
        // let arweave = Arweave::from_keypair_path(
        //     PathBuf::from(path),
        //     Url::from_str("https://arweave.net").unwrap(),
        // )
        // .await?;

        Ok(Uploader {})
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

    pub async fn upload(&self, path: &str) -> anyhow::Result<()> {
        let currency = Ar::new(PathBuf::from_str(path).unwrap(), None);
        let url = Url::parse("https://node1.bundlr.network").unwrap();
        let bundlr = Bundlr::new(url, &currency).await;

        let data = fs::read(self.fetch_latest_warc().unwrap().path()).expect("Could not read file");
        let mut tx = bundlr.create_transaction(data, self.create_tags());
        bundlr.sign_transaction(&mut tx).await?;
        let res = bundlr.send_transaction(tx).await?;
        println!("{}", res);
        Ok(())
    }

    fn create_tags(&self) -> Vec<Tag> {
        vec![
            Tag::new("App-Name", ""),
            Tag::new("App-Version", "0.0.1"),
            Tag::new("Content-Type", "application/json"),
        ]
    }
}
