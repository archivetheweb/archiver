use arloader::{
    transaction::{FromUtf8Strs, Tag},
    Arweave,
};
use futures::StreamExt;
use itertools::Itertools;
use std::{
    path::PathBuf,
    str::FromStr,
    sync::{self, Arc},
    time::Duration,
};
use tokio::fs;
use tokio_retry::{strategy::FixedInterval, Retry};

use anyhow::{anyhow, Context};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

use crate::{
    types::{ArchivingResult, BundlrUploadID, CrawlUploadResult},
    utils::{assert_stream_send, jitter, APP_NAME, APP_VERSION, BUNDLR_URL, WARC_APPLICATION_TYPE},
};

pub struct Uploader {
    _currency: String,
    arweave: Arweave,
    client: sync::Arc<reqwest::Client>,
}

#[derive(Serialize, Deserialize, Debug)]
struct BundlrRes {
    id: String,
    timestamp: u64,
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
            client: Arc::new(Client::new()),
        })
    }

    pub async fn upload_crawl_files(
        &self,
        archiving_result: &ArchivingResult,
    ) -> anyhow::Result<CrawlUploadResult> {
        let mut warc_file_ids = vec![];
        // TODO make these recursive bundles

        // first we do the warc files
        for file_path in &archiving_result.warc_files {
            let file_tx_id = self.upload_warc(file_path, &archiving_result).await?;
            warc_file_ids.push(file_tx_id);
        }
        // then the screenshot
        let screenshot_id = self
            .upload_screenshot(&archiving_result.screenshot_file, &archiving_result)
            .await?;

        Ok(CrawlUploadResult {
            screenshot_id: screenshot_id,
            warc_id: warc_file_ids,
        })
    }

    pub async fn upload_warc(
        &self,
        file_path: &PathBuf,
        archive_info: &ArchivingResult,
    ) -> anyhow::Result<String> {
        let data = fs::read(&file_path).await.context(format!(
            "upload_warc: could not read file at path {:?}",
            &file_path
        ))?;

        let mut tags = Self::append_app_tags(
            vec![],
            &archive_info.archive_info.url(),
            &archive_info.original_url,
            archive_info.archive_info.unix_ts(),
            archive_info.archive_info.depth(),
        );
        tags.push(Tag::<String>::from_utf8_strs("Content-Encoding", "gzip").unwrap());

        // first we deploy the file data
        let file_tx_id = self.upload_to_bundlr(data, tags).await?;

        return Ok(file_tx_id);
    }

    pub async fn upload_screenshot<'a>(
        &self,
        file_path: &PathBuf,
        archive_info: &ArchivingResult,
    ) -> anyhow::Result<String> {
        let screenshot_data = fs::read(&file_path).await.context(format!(
            "could not read screenshot_data at {:?}",
            &file_path
        ))?;

        // first we deploy the file data
        let screenshot_file_tx_id = self
            .upload_to_bundlr(
                screenshot_data,
                Self::append_app_tags(
                    vec![],
                    &archive_info.archive_info.url(),
                    &archive_info.original_url,
                    archive_info.archive_info.unix_ts(),
                    archive_info.archive_info.depth(),
                ),
            )
            .await?;

        return Ok(screenshot_file_tx_id);
    }

    async fn upload_to_bundlr(
        &self,
        data: Vec<u8>,
        tags: Vec<Tag<String>>,
    ) -> anyhow::Result<String> {
        let file_tx = self.arweave.create_data_item(data, tags, false)?;
        let file_tx = self.arweave.sign_data_item(file_tx)?;
        let file_tx_id = file_tx.id.to_string();

        let client = self.client.clone();

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
                Err(e) => {
                    return Err(anyhow!(
                        "could not send small bundle to bundlr {}",
                        e.to_string()
                    ))
                }
            }

            return Ok(file_tx_id);
        } else {
            // otherwise we need to chunk the data and send it
            debug!("Sending large bundles to Bundlr, chunking...");

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

            // we need to help the compiler with assert_stream_send
            // as we have a stream being awaited in multiple threads
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

    fn append_app_tags(
        mut tags: Vec<Tag<String>>,
        url: &str,
        original_url: &str,
        timestamp: i64,
        depth: u8,
    ) -> Vec<Tag<String>> {
        let mut t = vec![
            // App Tags
            Tag::<String>::from_utf8_strs("App-Name", &APP_NAME).unwrap(),
            Tag::<String>::from_utf8_strs("App-Version", &APP_VERSION).unwrap(),
            Tag::<String>::from_utf8_strs("Url", url.into()).unwrap(),
            Tag::<String>::from_utf8_strs("Original-Url", original_url.into()).unwrap(),
            Tag::<String>::from_utf8_strs("Timestamp", &format!("{}", timestamp)).unwrap(),
            Tag::<String>::from_utf8_strs("Crawl-Depth", &format!("{}", depth)).unwrap(),
            Tag::<String>::from_utf8_strs("Content-Type", WARC_APPLICATION_TYPE).unwrap(),
            Tag::<String>::from_utf8_strs(
                "Render-With",
                "m2s7Wam0E6PGuKcLHRkFQXo9ou4ASsEtdutlFKqbge8",
            )
            .unwrap(),
        ];
        tags.append(&mut t);
        return tags;
    }
}

const CHUNKING_THRESHOLD: usize = 50_000_000;

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
