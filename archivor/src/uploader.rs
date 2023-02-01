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

use crate::{
    types::{ArchiveInfo, ArchivingResult, CrawlUploadResult},
    utils::get_unix_timestamp,
};

pub struct Uploader {
    key_path: PathBuf,
    _currency: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct BundlrRes {
    id: String,
    timestamp: u64,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    data_encoding: Option<String>,
}

impl ArfsMetadata {
    pub fn new(
        name: String,
        size: usize,
        data_tx_id: String,
        data_content_type: String,
        data_encoding: Option<String>,
    ) -> Self {
        Self {
            name,
            size,
            data_tx_id,
            data_content_type,
            data_encoding,
            last_modified_date: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        }
    }
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
            _currency: currency.to_string(),
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

            let elems: Vec<&str> = file.to_str().unwrap().trim().split("_").collect();

            match elems[1].parse::<u128>() {
                Ok(ts) => ts,
                Err(_) => 0
            }
        }) else {
            return Err(anyhow!("problem reading the directory {:?}", directory));
        };
        Ok(latest.path())
    }

    pub async fn upload_crawl_files(
        &self,
        crawl: &ArchivingResult,
    ) -> anyhow::Result<CrawlUploadResult> {
        let currency = Ar::new(self.key_path.clone(), None);
        let bundlr = Bundlr::new(Url::parse(BUNDLR_URL).unwrap(), &currency).await;

        let mut warc_file_ids = vec![];
        let mut warc_metadata_ids = vec![];

        // first we do the warc files
        for file_path in &crawl.warc_files {
            let (file_tx_id, file_metadata_id) = self
                .upload_warc(&bundlr, file_path, &crawl.archive_info)
                .await?;
            warc_file_ids.push(file_tx_id);
            warc_metadata_ids.push(file_metadata_id);
        }
        // then the screenshot
        let (screenshot_id, screenshot_metadata_id) = self
            .upload_screenshot(&bundlr, &crawl.screenshot_file, &crawl.archive_info)
            .await?;

        Ok(CrawlUploadResult {
            screenshot_id: screenshot_id,
            screenshot_metadata_data_id: screenshot_metadata_id,
            warc_id: warc_file_ids,
            warc_metadata_data_id: warc_metadata_ids,
        })
    }

    pub async fn upload_warc<'a>(
        &self,
        bundlr: &Bundlr<'a>,
        file_path: &PathBuf,
        archive_info: &ArchiveInfo,
    ) -> anyhow::Result<(String, String)> {
        let data = fs::read(file_path)?;
        let name = match file_path.file_name() {
            Some(n) => n.to_str().unwrap(),
            None => return Err(anyhow!("invalid file path {:?}", file_path)),
        };

        let data_len = data.len();

        let mut tags = append_app_tags(
            create_arfs_file_data_tags(),
            &archive_info.url(),
            archive_info.unix_ts(),
            archive_info.depth(),
        );
        tags.push(Tag::new("Content-Encoding", "gzip"));

        // first we deploy the file data
        let mut file_tx = bundlr.create_transaction(data, tags);
        bundlr.sign_transaction(&mut file_tx).await?;

        let file_tx_id = get_bundle_id(file_tx.get_signarure());

        let metadata = ArfsMetadata::new(
            name.into(),
            data_len,
            file_tx_id.clone(),
            "application/warc".into(),
            Some("gzip".into()),
        );
        let mut mt_tags = append_app_tags(
            create_arfs_file_metadata_tags(),
            &archive_info.url(),
            archive_info.unix_ts(),
            archive_info.depth(),
        );

        mt_tags.push(Tag::new("Content-Encoding", "gzip"));

        let mut metadata_tx =
            bundlr.create_transaction(serde_json::to_vec(&metadata).unwrap(), mt_tags);
        bundlr.sign_transaction(&mut metadata_tx).await?;

        let metadata_tx_id = get_bundle_id(metadata_tx.get_signarure());
        let file_tx_res = bundlr.send_transaction(file_tx).await?;
        debug!("bundlr first tx {:?}", file_tx_res);
        let metadata_tx_res = bundlr.send_transaction(metadata_tx).await?;
        debug!("bundlr metadata tx {:?}", metadata_tx_res);

        return Ok((file_tx_id, metadata_tx_id));
    }

    pub async fn upload_screenshot<'a>(
        &self,
        bundlr: &Bundlr<'a>,
        file_path: &PathBuf,
        archive_info: &ArchiveInfo,
    ) -> anyhow::Result<(String, String)> {
        let screenshot_data = fs::read(file_path)?;
        let screenshot_name = match file_path.file_name() {
            Some(n) => n.to_str().unwrap(),
            None => return Err(anyhow!("screenshot: invalid file path {:?}", file_path)),
        };

        let sc_data_len = screenshot_data.len();

        // first we deploy the file data
        let mut screenshot_file_tx = bundlr.create_transaction(
            screenshot_data,
            append_app_tags(
                create_arfs_file_data_tags(),
                &archive_info.url(),
                archive_info.unix_ts(),
                archive_info.depth(),
            ),
        );
        bundlr.sign_transaction(&mut screenshot_file_tx).await?;

        let screenshot_file_tx_id = get_bundle_id(screenshot_file_tx.get_signarure());

        let metadata = ArfsMetadata::new(
            screenshot_name.into(),
            sc_data_len,
            screenshot_file_tx_id.clone(),
            "image/png".into(),
            None,
        );

        let mut screenshot_metadata_tx = bundlr.create_transaction(
            serde_json::to_vec(&metadata).unwrap(),
            append_app_tags(
                create_arfs_file_metadata_tags(),
                &archive_info.url(),
                archive_info.unix_ts(),
                archive_info.depth(),
            ),
        );
        bundlr.sign_transaction(&mut screenshot_metadata_tx).await?;

        let screenshot_metadata_tx_id = get_bundle_id(screenshot_metadata_tx.get_signarure());
        let file_tx_res = bundlr.send_transaction(screenshot_file_tx).await?;
        debug!("screenshot bundlr first tx {:?}", file_tx_res);
        let metadata_tx_res = bundlr.send_transaction(screenshot_metadata_tx).await?;
        debug!("screenshot bundlr metadata tx {:?}", metadata_tx_res);

        return Ok((screenshot_file_tx_id, screenshot_metadata_tx_id));
    }
}

fn append_app_tags(mut tags: Vec<Tag>, url: &str, timestamp: i64, depth: u8) -> Vec<Tag> {
    let mut t = vec![
        // App Tags
        Tag::new("App-Name", "atw"),
        Tag::new("App-Version", "0.0.1_beta"),
        Tag::new("Url", url.into()),
        Tag::new("Timestamp", &format!("{}", timestamp)),
        Tag::new("Crawl-Depth", &format!("{}", depth)),
    ];
    tags.append(&mut t);
    return tags;
}

fn create_arfs_file_metadata_tags() -> Vec<Tag> {
    vec![
        // Ardrive FS tags
        Tag::new("ArFS", "0.11"),
        Tag::new("App-Version", "0.0.1_beta"),
        Tag::new("Content-Type", "application/json"),
        Tag::new("Drive-Id", DRIVE_ID),
        Tag::new("Entity-Type", "file"),
        Tag::new("File-Id", &Uuid::new_v4().to_string()),
        Tag::new("Parent-Folder-Id", PARENT_FOLDER_ID),
        Tag::new("Unix-Time", &get_unix_timestamp().as_secs().to_string()),
    ]
}

fn create_arfs_file_data_tags() -> Vec<Tag> {
    vec![
        // Ardive FS tags
        Tag::new("Content-Type", "application/warc"),
    ]
}

fn get_bundle_id(signature: Vec<u8>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(signature);
    let result = hasher.finalize();
    general_purpose::URL_SAFE_NO_PAD.encode(result)
}
