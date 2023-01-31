use std::path::PathBuf;

#[derive(Debug)]
pub struct CrawlUploadResult {
    pub screenshot_id: String,
    pub screenshot_metadata_data_id: String,
    pub warc_id: Vec<String>,
    pub warc_metadata_data_id: Vec<String>,
}

#[derive(Debug)]
pub struct CrawlResult {
    pub warc_files: Vec<PathBuf>,
    pub screenshot_file: PathBuf,
    pub timestamp: String,
    pub depth: i32,
    pub domain: String,
}
