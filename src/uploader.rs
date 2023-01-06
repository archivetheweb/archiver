use std::{
    fmt::Error,
    fs::{self, DirEntry},
    path::PathBuf,
    str::FromStr,
};

use crate::utils::{ARCHIVE_DIR, BASE_DIR};
use anyhow::anyhow;
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

pub struct Uploader {
    key_path: PathBuf,
    currency: String,
}

const BUNDLR_URL: &str = "https://node1.bundlr.network";

impl Uploader {
    pub async fn new(path: &str, currency: &str) -> anyhow::Result<Self> {
        if currency != "arweave" {
            return Err(anyhow!("arweave is the only supported currency"));
        }
        Ok(Uploader {
            key_path: PathBuf::from(path),
            currency: currency.to_string(),
        })
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

    pub async fn upload_latest(&self) -> anyhow::Result<()> {
        let currency = Ar::new(self.key_path.clone(), None);
        let url = Url::parse(BUNDLR_URL).unwrap();
        let bundlr = Bundlr::new(url, &currency).await;

        let data_path = self.fetch_latest_warc()?.path();
        let data = fs::read(data_path)?;

        let mut tx = bundlr.create_transaction(data, self.create_tags());
        bundlr.sign_transaction(&mut tx).await?;
        let res = bundlr.send_transaction(tx).await?;
        println!("{}", res);
        Ok(())
    }

    fn create_tags(&self) -> Vec<Tag> {
        vec![
            Tag::new("App-Name", "atw"),
            Tag::new("App-Version", "0.0.1_beta"),
            Tag::new("Content-Type", "application/warc"),
        ]
    }
}
