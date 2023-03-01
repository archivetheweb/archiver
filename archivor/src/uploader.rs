use arloader::{
    transaction::{FromUtf8Strs, Tag},
    Arweave,
};
use futures::StreamExt;
use itertools::Itertools;
use std::{
    path::PathBuf,
    str::FromStr,
    time::{Duration, SystemTime},
};
use tokio::fs;
use tokio_retry::{strategy::FixedInterval, Retry};

use anyhow::anyhow;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    types::{
        ArchiveInfo, ArchivingResult, CrawlUploadResult, BUNDLR_URL, DRIVE_ID, PARENT_FOLDER_ID,
    },
    utils::{assert_stream_send, get_unix_timestamp, jitter, WARC_APPLICATION_TYPE},
};

pub struct Uploader {
    _currency: String,
    arweave: Arweave,
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
        let arweave = Arweave::from_keypair_path(
            key_path.clone(),
            Url::from_str("https://arweave.net").unwrap(),
        )
        .await?;

        Ok(Uploader {
            _currency: currency.to_string(),
            arweave,
        })
    }

    pub async fn upload_crawl_files(
        &self,
        crawl: &ArchivingResult,
    ) -> anyhow::Result<CrawlUploadResult> {
        let mut warc_file_ids = vec![];
        let mut warc_metadata_ids = vec![];

        // TODO make these recursive bundles

        // first we do the warc files
        for file_path in &crawl.warc_files {
            let (file_tx_id, file_metadata_id) =
                self.upload_warc(file_path, &crawl.archive_info).await?;
            warc_file_ids.push(file_tx_id);
            warc_metadata_ids.push(file_metadata_id);
        }
        // then the screenshot
        let (screenshot_id, screenshot_metadata_id) = self
            .upload_screenshot(&crawl.screenshot_file, &crawl.archive_info)
            .await?;

        Ok(CrawlUploadResult {
            screenshot_id: screenshot_id,
            screenshot_metadata_data_id: screenshot_metadata_id,
            warc_id: warc_file_ids,
            warc_metadata_data_id: warc_metadata_ids,
        })
    }

    pub async fn upload_warc(
        &self,
        file_path: &PathBuf,
        archive_info: &ArchiveInfo,
    ) -> anyhow::Result<(String, String)> {
        let data = fs::read(file_path).await?;
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
        tags.push(Tag::<String>::from_utf8_strs("Content-Encoding", "gzip").unwrap());

        // first we deploy the file data
        let file_tx_id = self.upload_to_bundlr(data, tags).await?;

        let metadata = ArfsMetadata::new(
            name.into(),
            data_len,
            file_tx_id.clone(),
            WARC_APPLICATION_TYPE.into(),
            Some("gzip".into()),
        );
        let mut mt_tags = append_app_tags(
            create_arfs_file_metadata_tags(),
            &archive_info.url(),
            archive_info.unix_ts(),
            archive_info.depth(),
        );
        mt_tags.push(Tag::<String>::from_utf8_strs("Content-Encoding", "gzip").unwrap());

        // then the metadata
        let metadata_tx_id = self
            .upload_to_bundlr(serde_json::to_vec(&metadata).unwrap(), mt_tags)
            .await?;

        return Ok((file_tx_id, metadata_tx_id));
    }

    pub async fn upload_screenshot<'a>(
        &self,
        file_path: &PathBuf,
        archive_info: &ArchiveInfo,
    ) -> anyhow::Result<(String, String)> {
        let screenshot_data = fs::read(file_path).await?;
        let screenshot_name = match file_path.file_name() {
            Some(n) => n.to_str().unwrap(),
            None => return Err(anyhow!("screenshot: invalid file path {:?}", file_path)),
        };

        let sc_data_len = screenshot_data.len();

        // first we deploy the file data
        let screenshot_file_tx_id = self
            .upload_to_bundlr(
                screenshot_data,
                append_app_tags(
                    create_arfs_file_data_tags(),
                    &archive_info.url(),
                    archive_info.unix_ts(),
                    archive_info.depth(),
                ),
            )
            .await?;

        let metadata = ArfsMetadata::new(
            screenshot_name.into(),
            sc_data_len,
            screenshot_file_tx_id.clone(),
            "image/png".into(),
            None,
        );

        let screenshot_metadata_tx_id = self
            .upload_to_bundlr(
                serde_json::to_vec(&metadata).unwrap(),
                append_app_tags(
                    create_arfs_file_metadata_tags(),
                    &archive_info.url(),
                    archive_info.unix_ts(),
                    archive_info.depth(),
                ),
            )
            .await?;

        return Ok((screenshot_file_tx_id, screenshot_metadata_tx_id));
    }

    async fn upload_to_bundlr(
        &self,
        data: Vec<u8>,
        tags: Vec<Tag<String>>,
    ) -> anyhow::Result<String> {
        let file_tx = self.arweave.create_data_item(data, tags, false)?;
        let file_tx = self.arweave.sign_data_item(file_tx)?;
        let file_tx_id = file_tx.id.to_string();

        let client = reqwest::Client::new();

        let data = file_tx.serialize()?;
        let size = data.len();

        // if the data size if small, we can send it straight to bundlr
        if size < CHUNKING_THRESHOLD {
            match client
                .post(format!("{}/tx/arweave", BUNDLR_URL))
                .header("Content-Type", "application/octet-stream")
                .body(file_tx.serialize().unwrap())
                .send()
                .await
            {
                Ok(res) => {
                    let res = res.text().await.unwrap();
                    debug!("{res}")
                }
                Err(e) => return Err(anyhow!(e.to_string())),
            }

            return Ok(file_tx_id);
        } else {
            // otherwise we need to chunk the data and send it
            let upload_info = client
                .get(format!("{}/chunks/arweave/-1/-1", BUNDLR_URL))
                .header("x-chunking-version", "2")
                .send()
                .await?;
            let upload_info = upload_info.json::<BundlrUploadID>().await?;
            let upload_id = upload_info.id;

            debug!("Upload ID: {}", upload_id);

            if size < upload_info.min || size > upload_info.max {
                return Err(anyhow!(
                    "Chunk size out of allowed range: {} - {}, currently {}",
                    upload_info.min,
                    upload_info.max,
                    size
                ));
            }

            let data = data
                .into_iter()
                .chunks(upload_info.min)
                .into_iter()
                .map(|x| x.collect::<Vec<u8>>())
                .collect::<Vec<Vec<u8>>>();

            let mut stream = assert_stream_send(
                tokio_stream::iter(data.iter().enumerate())
                    .map(|p| {
                        let retry_strategy = FixedInterval::from_millis(20)
                            .map(jitter) // add jitter to delays
                            .take(5);
                        let index = p.0;
                        let uid = upload_id.clone();
                        let client = client.clone();
                        Retry::spawn(retry_strategy, move || {
                            client
                                .post(format!("{}/chunks/arweave/{}/{}", BUNDLR_URL, uid, index))
                                .header("Content-Type", "application/octet-stream")
                                .header("x-chunking-version", "2")
                                .timeout(Duration::from_secs(20))
                                .body(p.1.clone())
                                .send()
                        })
                    })
                    .buffer_unordered(10),
            );

            let mut counter = 0;
            while let Some(result) = stream.next().await {
                match result {
                    Ok(res) => {
                        println!("{:?}", res.text().await);
                        if counter == 0 {
                            debug!("{}", "started");
                        }
                        counter += 1;
                    }
                    Err(e) => {
                        return Err(anyhow!("could not upload chunk with error: {}", e));
                    }
                }
            }
            debug!("Uploaded {} chunks", counter);

            let finish = client
                .post(format!("{}/chunks/arweave/{}/-1", BUNDLR_URL, upload_id))
                .header("x-chunking-version", "2")
                .header("Content-Type", "application/octet-stream")
                .timeout(Duration::from_secs(40))
                .send()
                .await?;

            let status = finish.status();
            let res = finish.text().await?;

            if status.as_u16() >= 300 {
                return Err(anyhow!(res));
            }

            debug!(
                "Successfully uploaded tx \n Status: {:#} \n Response: {:#}",
                status, res
            );

            return Ok(file_tx_id);
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct BundlrUploadID {
    pub id: String,
    pub min: usize,
    pub max: usize,
}
const CHUNKING_THRESHOLD: usize = 50_000_000;

fn append_app_tags(
    mut tags: Vec<Tag<String>>,
    url: &str,
    timestamp: i64,
    depth: u8,
) -> Vec<Tag<String>> {
    let mut t = vec![
        // App Tags
        Tag::<String>::from_utf8_strs("App-Name", "atw").unwrap(),
        Tag::<String>::from_utf8_strs("App-Version", "0.0.1_beta").unwrap(),
        Tag::<String>::from_utf8_strs("Url", url.into()).unwrap(),
        Tag::<String>::from_utf8_strs("Timestamp", &format!("{}", timestamp)).unwrap(),
        Tag::<String>::from_utf8_strs("Crawl-Depth", &format!("{}", depth)).unwrap(),
    ];
    tags.append(&mut t);
    return tags;
}

fn create_arfs_file_metadata_tags() -> Vec<Tag<String>> {
    vec![
        // Ardrive FS tags
        Tag::<String>::from_utf8_strs("ArFS", "0.11").unwrap(),
        Tag::<String>::from_utf8_strs("App-Version", "0.0.1_beta").unwrap(),
        Tag::<String>::from_utf8_strs("Content-Type", "application/json").unwrap(),
        Tag::<String>::from_utf8_strs("Drive-Id", DRIVE_ID).unwrap(),
        Tag::<String>::from_utf8_strs("Entity-Type", "file").unwrap(),
        Tag::<String>::from_utf8_strs("File-Id", &Uuid::new_v4().to_string()).unwrap(),
        Tag::<String>::from_utf8_strs("Parent-Folder-Id", PARENT_FOLDER_ID).unwrap(),
        Tag::<String>::from_utf8_strs("Unix-Time", &get_unix_timestamp().as_secs().to_string())
            .unwrap(),
    ]
}

fn create_arfs_file_data_tags() -> Vec<Tag<String>> {
    vec![
        // Ardive FS tags
        Tag::<String>::from_utf8_strs("Content-Type", WARC_APPLICATION_TYPE).unwrap(),
    ]
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    #[ignore]
    fn test_upload_large_data_item() {
        let u = tokio_test::block_on(Uploader::new(
            PathBuf::from_str(".secret/test_wallet.json").unwrap(),
            "arweave",
        ))
        .unwrap();

        let d = tokio_test::block_on(fs::read("res/5MB.zip")).unwrap();

        let r = tokio_test::block_on(u.upload_to_bundlr(d, vec![])).unwrap();

        println!("{:?}", r)
    }
}
