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
    pub warc_id: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BundlrBalance {
    pub balance: String,
}

#[derive(Debug)]
pub struct CrawlResult {
    url: String,
    main_title: String,
    visited: HashSet<String>,
    failed: HashSet<String>,
}

impl CrawlResult {
    pub fn new(
        url: String,
        main_title: String,
        visited: HashSet<String>,
        failed: HashSet<String>,
    ) -> Self {
        CrawlResult {
            url,
            main_title,
            visited: visited,
            failed: failed,
        }
    }

    pub fn url(&self) -> String {
        self.url.clone()
    }
    pub fn main_title(&self) -> String {
        self.main_title.clone()
    }
    pub fn visited(&self) -> HashSet<String> {
        self.visited.clone()
    }
    pub fn failed(&self) -> HashSet<String> {
        self.failed.clone()
    }
}

#[derive(Debug)]
pub struct ArchivingResult {
    pub warc_files: Vec<PathBuf>,
    pub screenshot_file: PathBuf,
    pub archive_info: ArchiveInfo,
    pub title: String,
    pub original_url: String,
}

#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Debug, Clone)]
pub struct UrlInfo {
    pub url: String,
    pub domain: String,
}

#[derive(Debug)]
pub struct PageCrawlResult {
    visited_url: String,
    links: Vec<UrlInfo>,
    depth: i32,
}

#[derive(Debug, Clone)]
pub struct CrawlRequest {
    url: String,
    depth: i32,
}

impl CrawlRequest {
    pub fn new(url: String, depth: i32) -> Self {
        CrawlRequest { url, depth }
    }

    pub fn url(&self) -> String {
        self.url.clone()
    }

    pub fn depth(&self) -> i32 {
        self.depth
    }
}

impl PageCrawlResult {
    pub fn new(visited_url: String, links: Vec<UrlInfo>, depth: i32) -> Self {
        PageCrawlResult {
            visited_url,
            links,
            depth,
        }
    }

    pub fn visited_url(&self) -> String {
        self.visited_url.clone()
    }

    pub fn links(&self) -> Vec<UrlInfo> {
        self.links.clone()
    }

    pub fn depth(&self) -> i32 {
        self.depth
    }
}

impl PartialEq for UrlInfo {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}
impl Eq for UrlInfo {}

impl std::hash::Hash for UrlInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.url.hash(state);
    }
}

pub struct BrowsingResult {
    links: Vec<UrlInfo>,
    pub error: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl BrowsingResult {
    pub fn new(
        links: Vec<UrlInfo>,
        error: Option<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        BrowsingResult { links, error }
    }

    pub fn links(&self) -> Vec<UrlInfo> {
        self.links.clone()
    }
}
