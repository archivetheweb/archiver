use sha2::{Digest, Sha256};
use std::{
    fs::{self},
    path::PathBuf,
    time::SystemTime,
};

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
    pub async fn new(key_path: PathBuf, currency: &str) -> anyhow::Result<Self> {
        if currency != "arweave" {
            return Err(anyhow!("arweave is the only supported currency"));
        }

        if !key_path.exists() {
            return Err(anyhow!(
                "could not read arweave key path: {}",
                key_path.to_str().unwrap()
            ));
        }

        Ok(Uploader {
            key_path,
            currency: currency.to_string(),
        })
    }

    pub fn fetch_latest_warc(&self, directory: &PathBuf) -> anyhow::Result<PathBuf> {
        let dir = fs::read_dir(directory)?;
        let Some(latest) = dir.into_iter().filter_map(|x| {
            match x {
                Ok(x) => {
                    if x.file_name().to_str().unwrap().contains("<unprocessed>") {
                        return None
                    }
                    Some(x)
                }
                Err(e)=>{
                    error!("could not filter map for fetch_latest_warc {}", e);
                    None
                }
            }
        }).max_by_key(|x| {
            let file = x.file_name();

            let elems: Vec<&str> = file.to_str().unwrap().trim().split("-").collect();

            match elems[1].parse::<u128>() {
                Ok(ts) => ts,
                Err(_) => 0
            }
        }) else {
            return Err(anyhow!("problem reading the directory {:?}", directory));
        };
        Ok(latest.path())
    }

    pub async fn upload_latest_file(
        &self,
        directory: &PathBuf,
    ) -> anyhow::Result<(String, String)> {
        // get the latest file
        let latest_file_path = self.fetch_latest_warc(directory)?;
        self.upload(&latest_file_path).await
    }

    // TODO
    // pub async fn upload_dir()

    pub async fn upload(&self, file_path: &PathBuf) -> anyhow::Result<(String, String)> {
        if self.currency == "arweave" {
            let currency = Ar::new(self.key_path.clone(), None);
            let bundlr = Bundlr::new(Url::parse(BUNDLR_URL).unwrap(), &currency).await;

            let data = fs::read(file_path)?;
            let name = match file_path.file_name() {
                Some(n) => n.to_str().unwrap(),
                None => return Err(anyhow!("invalid file path {:?}", file_path)),
            };
            //archivoor_<ts>_<url>_<depth>.warc.gz
            let elems = name.split("_").collect::<Vec<&str>>();
            let ts = elems[1];
            let url = elems[2];
            let depth = elems[3];
            let data_len = data.len();

            // first we deploy the file data
            let mut file_tx =
                bundlr.create_transaction(data, create_file_data_tags(url, ts, depth));
            bundlr.sign_transaction(&mut file_tx).await?;

            let file_tx_id = get_bundle_id(file_tx.get_signarure());

            let metadata = ArfsMetadata {
                name: name.into(),
                size: data_len,
                last_modified_date: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
                data_tx_id: file_tx_id.clone(),
                data_content_type: "application/warc".into(),
                data_encoding: "gzip".into(),
            };

            let mut metadata_tx = bundlr.create_transaction(
                serde_json::to_vec(&metadata).unwrap(),
                create_file_metadata_tags(url, ts, depth),
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
    last_modified_date: u128,
    #[serde(rename = "dataTxId")]
    data_tx_id: String,
    #[serde(rename = "dataContentType")]
    data_content_type: String,
    #[serde(rename = "dataEncoding")]
    data_encoding: String,
}

fn create_file_metadata_tags(url: &str, timestamp: &str, depth: &str) -> Vec<Tag> {
    vec![
        // Ardrive FS tags
        Tag::new("ArFS", "0.11"),
        Tag::new("App-Version", "0.0.1_beta"),
        Tag::new("Content-Type", "application/json"),
        Tag::new("Drive-Id", DRIVE_ID),
        Tag::new("Entity-Type", "file"),
        Tag::new("File-Id", &Uuid::new_v4().to_string()),
        Tag::new("Parent-Folder-Id", PARENT_FOLDER_ID),
        Tag::new("Unix-Time", &get_unix_timestamp().to_string()),
        // App Tags
        Tag::new("App-Name", "atw"),
        Tag::new("Url", url.into()),
        Tag::new("Timestamp", timestamp.into()),
        Tag::new("Crawl-Depth", depth.into()),
    ]
}

fn create_file_data_tags(url: &str, timestamp: &str, depth: &str) -> Vec<Tag> {
    vec![
        // Ardive FS tags
        Tag::new("Content-Type", "application/warc"),
        // App tags
        Tag::new("App-Name", "atw"),
        Tag::new("App-Version", "0.0.1_beta"),
        Tag::new("Content-Encoding", "gzip"),
        Tag::new("Url", url.into()),
        Tag::new("Timestamp", timestamp.into()),
        Tag::new("Crawl-Depth", depth.into()),
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
