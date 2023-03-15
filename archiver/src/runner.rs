use std::{
    path::PathBuf,
    str::FromStr,
    sync::{
        Arc,
        {atomic::AtomicBool, atomic::Ordering},
    },
};

use anyhow::{anyhow, Context};
use reqwest::Url;
use signal_hook::consts::{SIGINT, SIGTERM};

use crate::{
    crawler::Crawler,
    types::{ArchiveInfo, ArchivingResult, CrawlUploadResult},
    uploader::Uploader,
    utils::BASE_URL,
    warc_writer::WarcWriter,
};

pub struct Runner {
    uploader: Option<Uploader>,
    warc_writer: WarcWriter,
    options: RunnerOptions,
    should_terminate: Arc<AtomicBool>,
}

#[derive(Builder, Debug)]
#[builder(setter(into))]
pub struct RunnerOptions {
    #[builder(default = "Some(8080)")]
    // port for the warc_writer
    writer_port: Option<u16>,
    // directory where the warc files will be written to
    #[builder(default = "self.default_writer_dir()")]
    writer_dir: Option<PathBuf>,
    // writer toggle for debug mode
    #[builder(default = "false")]
    writer_debug: bool,
    // depth of crawl (0=page only, 1=page+links, 2=page+links+links in linked pages)
    #[builder(default = "1")]
    crawl_depth: i32,
    // whether to only grab links in the domain or not
    #[builder(default = "false")]
    domain_only: bool,
    #[builder(default = "5")]
    concurrent_tabs: i32,
    #[builder(default = "2")]
    url_retries: i32,
    // minimum wait time after navigation in seconds
    #[builder(default = "5")]
    min_wait_after_navigation: u64,
    // maximum wait time after navigation in seconds
    #[builder(default = "7")]
    max_wait_after_navigation: u64,
    // browser timeout in seconds
    #[builder(default = "45")]
    timeout: u64,
    // base url where the warc writer can be accessed
    #[builder(default = "self.default_base_url()")]
    base_url: String,
    // optional setting to change the archive name
    #[builder(default = "self.default_archive_name()")]
    archive_name: Option<String>,
    // toggle if we want to upload the finished warc files to arweave
    #[builder(default = "false")]
    with_upload: bool,
    // path to the arweave keyfile
    #[builder(default = "self.default_arweave_wallet_dir()")]
    arweave_key_dir: PathBuf,
    // currency to pay out for the bundlr service
    #[builder(default = "self.default_currency()")]
    currency: String,
}

impl RunnerOptions {
    pub fn default_builder() -> RunnerOptionsBuilder {
        RunnerOptionsBuilder::default()
    }
}

impl RunnerOptionsBuilder {
    fn default_archive_name(&self) -> Option<String> {
        Some(String::from("archiver"))
    }
    fn default_base_url(&self) -> String {
        BASE_URL.into()
    }
    fn default_writer_dir(&self) -> Option<PathBuf> {
        Some(PathBuf::from(format!("")))
    }
    fn default_arweave_wallet_dir(&self) -> PathBuf {
        PathBuf::from(".secret/wallet.json")
    }
    fn default_currency(&self) -> String {
        String::from("arweave")
    }
}

impl Runner {
    pub async fn new(lo: RunnerOptions) -> anyhow::Result<Self> {
        let warc_writer = WarcWriter::new(
            lo.writer_port,
            lo.writer_dir.clone(),
            lo.archive_name.clone(),
            lo.writer_debug,
        )?;

        let uploader = if lo.with_upload {
            let u = Uploader::new(lo.arweave_key_dir.clone(), &lo.currency)
                .await
                .context("could not instantiate uploader")?;
            Some(u)
        } else {
            None
        };

        let should_terminate = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate))?;
        signal_hook::flag::register(SIGINT, Arc::clone(&should_terminate))?;

        Ok(Runner {
            uploader,
            warc_writer,
            options: lo,
            should_terminate,
        })
    }

    pub async fn run_all(&self, url: &str) -> anyhow::Result<()> {
        let crawl = self.run_archiving(url).await?;

        if !self.should_terminate.load(Ordering::Relaxed) {
            self.run_upload_crawl(crawl).await?;
        }

        Ok(())
    }

    fn prepare_urls(&self, url: &str) -> anyhow::Result<(String, String, String)> {
        let u = Url::from_str(url).context(format!("url passed is invalid {}", url))?;
        let domain = match u.domain() {
            Some(d) => d,
            None => return Err(anyhow!("url must have a valid domain")),
        };
        let base_url = format!("{}:{}", self.options.base_url, self.warc_writer.port());

        let full_url = format!(
            "{}/{}/record/{}",
            base_url,
            self.warc_writer.archive_name(),
            url
        );

        Ok((base_url, full_url, domain.into()))
    }

    pub async fn run_archiving(&self, original_url: &str) -> anyhow::Result<ArchivingResult> {
        let (base_url, full_url, domain) = self.prepare_urls(original_url)?;

        info!(
            "initializing crawl of {} with depth {}, {} browsers, domain_only: {} and {} retries.",
            original_url,
            self.options.crawl_depth,
            self.options.concurrent_tabs,
            self.options.domain_only,
            self.options.url_retries
        );
        let mut crawler = Crawler::new(
            &base_url,
            &full_url,
            &original_url,
            self.options.crawl_depth,
            self.options.domain_only,
            self.options.concurrent_tabs,
            self.options.url_retries,
            self.options.timeout,
            self.options.min_wait_after_navigation,
            self.options.max_wait_after_navigation,
        );
        let crawl = crawler.crawl(self.should_terminate.clone()).await?;

        // we rename the files that the warc writer created for easy retrieval
        let files = self
            .warc_writer
            .rename_warc_files(&domain, self.options.crawl_depth)?;

        let archive_info = ArchiveInfo::new(&files[0])?;

        let screenshot_dir = self.warc_writer.process_screenshot(
            &archive_info.string_ts(),
            &domain,
            self.options.crawl_depth,
        )?;

        Ok(ArchivingResult {
            warc_files: files,
            screenshot_file: screenshot_dir,
            archive_info: archive_info,
            title: crawl.main_title,
            original_url: original_url.into(),
        })
    }

    pub async fn run_upload_crawl(
        &self,
        crawl: ArchivingResult,
    ) -> anyhow::Result<CrawlUploadResult> {
        if !self.options.with_upload {
            return Err(anyhow!("with_upload is false"));
        }

        match &self.uploader {
            Some(u) => {
                let ids = u.upload_crawl_files(&crawl).await?;

                return Ok(ids);
            }
            None => Err(anyhow!("uploader not defined")),
        }
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        debug!("{}", "terminating runner...");
        self.warc_writer.terminate().unwrap();
        debug!("{}", "warc_writer child process killed, goodbye");
    }
}
