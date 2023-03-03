use std::{collections::HashSet, path::PathBuf};

use crate::utils::FORMAT_STRING;
use anyhow::anyhow;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArchiverError {
    #[error("contract_interaction: {0}")]
    ContractInteractionError(String),
    #[error("early_termination")]
    EarlyTermination,
}

#[derive(Debug)]
pub struct CrawlUploadResult {
    pub screenshot_id: String,
    pub screenshot_metadata_data_id: String,
    pub warc_id: Vec<String>,
    pub warc_metadata_data_id: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BundlrBalance {
    pub balance: String,
}

#[derive(Debug)]
pub struct CrawlResult {
    pub url: String,
    pub main_title: String,
    pub visited: HashSet<String>,
    pub failed: HashSet<String>,
}

#[derive(Debug)]
pub struct ArchivingResult {
    pub warc_files: Vec<PathBuf>,
    pub screenshot_file: PathBuf,
    pub archive_info: ArchiveInfo,
    pub title: String,
}

#[derive(Serialize, Deserialize)]
pub struct BundlrUploadID {
    pub id: String,
    pub min: usize,
    pub max: usize,
}

#[derive(Debug)]
pub struct ArchiveInfo {
    depth: u8,
    timestamp: NaiveDateTime,
    url: String,
}

impl ArchiveInfo {
    pub fn new(file: &PathBuf) -> anyhow::Result<Self> {
        Self::get_archive_information_from_name(file)
    }

    pub fn depth(&self) -> u8 {
        self.depth
    }

    pub fn url(&self) -> String {
        self.url.clone()
    }

    pub fn unix_ts(&self) -> i64 {
        self.timestamp.timestamp()
    }

    pub fn string_ts(&self) -> String {
        self.timestamp.format(FORMAT_STRING).to_string()
    }

    fn get_archive_information_from_name(filename: &PathBuf) -> anyhow::Result<ArchiveInfo> {
        let file_path = PathBuf::from(filename);
        let name = match file_path.file_name() {
            Some(n) => n.to_str().unwrap(),
            None => return Err(anyhow!("invalid file path {:?}", file_path)),
        };

        //archiver_<ts>_<url>_<depth>.warc.gz
        let elems = name.split("_").collect::<Vec<&str>>();

        let depth: u8 = elems[3].split_once(".").unwrap().0.parse()?;

        let ts = NaiveDateTime::parse_from_str(elems[1], FORMAT_STRING)?;

        let url = elems[2];

        Ok(ArchiveInfo {
            depth: depth,
            timestamp: ts,
            url: url.into(),
        })
    }
}
