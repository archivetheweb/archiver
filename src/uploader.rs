use sha2::{Digest, Sha256};
use std::{
    fs::{self},
    path::PathBuf,
    time::SystemTime,
};

use crate::utils::{ARCHIVE_DIR, BASE_DIR};
use anyhow::anyhow;
use base64::{engine::general_purpose, Engine as _};
use bundlr_sdk::{currency::arweave::Arweave as Ar, tags::Tag, Bundlr};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub struct Uploader {
    key_path: PathBuf,
    currency: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct BundlrRes {
    id: String,
    timestamp: u64,
}

const BUNDLR_URL: &str = "https://node1.bundlr.network";

const DRIVE_ID: &str = "b7db009e-dd28-4546-ba5f-d091e09e2d6e";
const PARENT_FOLDER_ID: &str = "62afa694-5260-4553-bf39-e09c65a52d9d";

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
    pub fn fetch_latest_warc(&self) -> anyhow::Result<(String, PathBuf)> {
        let dir = fs::read_dir(format!("./{}/{}/archive", BASE_DIR, ARCHIVE_DIR))?;

        let latest = dir.into_iter().map(|x| x.unwrap()).max_by_key(|x| {
            let file = x.file_name();

            let elems: Vec<&str> = file.to_str().unwrap().trim().split("-").collect();

            elems[1].parse::<u128>().unwrap()
        });

        let latest = latest.unwrap();
        let name = latest.file_name();
        let name = name.to_str().unwrap();
        let path = latest.path();

        Ok((String::from(name), path))
    }

    pub async fn upload_latest(&self) -> anyhow::Result<(String, String)> {
        if self.currency == "arweave" {
            let currency = Ar::new(self.key_path.clone(), None);
            let bundlr = Bundlr::new(Url::parse(BUNDLR_URL).unwrap(), &currency).await;

            let (name, path) = self.fetch_latest_warc()?;
            let data = fs::read(path)?;
            let data_len = data.len();

            // first we deploy the file data
            let mut file_tx = bundlr.create_transaction(data, create_file_data_tags());
            bundlr.sign_transaction(&mut file_tx).await?;

            let file_tx_id = get_bundle_id(file_tx.get_signarure());

            let metadata = ArfsMetadata {
                name: name,
                size: data_len,
                last_modified_date: get_unix_timestamp(),
                data_tx_id: file_tx_id.clone(),
                data_content_type: "application/warc".into(),
            };

            let mut metadata_tx = bundlr.create_transaction(
                serde_json::to_vec(&metadata).unwrap(),
                create_file_metadata_tags(),
            );
            bundlr.sign_transaction(&mut metadata_tx).await?;

            let metadata_tx_id = get_bundle_id(metadata_tx.get_signarure());
            let file_tx_res = bundlr.send_transaction(file_tx).await?;
            debug!("bundlr first tx {:?}", file_tx_res);
            let metadata_tx_res = bundlr.send_transaction(metadata_tx).await?;
            debug!("bundlr metadata tx {:?}", metadata_tx_res);

            return Ok((file_tx_id, metadata_tx_id));
        }
        Err(anyhow!("not supported yet"))
    }
}

#[derive(Serialize, Deserialize)]
struct ArfsMetadata {
    name: String,
    size: usize,
    #[serde(rename = "lastModifiedDate")]
    last_modified_date: u64,
    #[serde(rename = "dataTxId")]
    data_tx_id: String,
    #[serde(rename = "dataContentType")]
    data_content_type: String,
}

fn create_file_metadata_tags() -> Vec<Tag> {
    vec![
        Tag::new("ArFS", "0.11"),
        Tag::new("App-Name", "ArDrive-App"),
        Tag::new("App-Version", "0.0.1_beta"),
        Tag::new("Content-Type", "application/json"),
        Tag::new("Drive-Id", DRIVE_ID),
        Tag::new("Entity-Type", "file"),
        Tag::new("File-Id", &Uuid::new_v4().to_string()),
        Tag::new("Parent-Folder-Id", PARENT_FOLDER_ID),
        Tag::new("Unix-Time", &get_unix_timestamp().to_string()),
    ]
}

fn create_file_data_tags() -> Vec<Tag> {
    vec![
        Tag::new("App-Name", "ArDrive-App"),
        Tag::new("App-Version", "0.0.1_beta"),
        Tag::new("Content-Type", "application/warc"),
        Tag::new("Content-Encoding", "gzip"),
    ]
}

fn get_bundle_id(signature: Vec<u8>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(signature);
    let result = hasher.finalize();
    general_purpose::URL_SAFE_NO_PAD.encode(result)
}

fn get_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
