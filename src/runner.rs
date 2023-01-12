use std::{
    path::PathBuf,
    str::FromStr,
    sync::{
        mpsc::{sync_channel, TryRecvError},
        Arc,
        {atomic::AtomicBool, atomic::Ordering},
    },
    thread::sleep,
    time::Duration,
};

use anyhow::anyhow;
use reqwest::Url;
use signal_hook::consts::{SIGINT, SIGTERM};

use crate::{crawler::Crawler, uploader::Uploader, utils::BASE_URL, warc_writer::WarcWriter};

pub struct Runner {
    uploader: Option<Uploader>,
    warc_writer: WarcWriter,
    options: LaunchOptions,
}

#[derive(Builder, Debug)]
#[builder(setter(into))]
pub struct LaunchOptions {
    #[builder(default = "Some(8080)")]
    writer_port: Option<u16>,
    #[builder(default = "self.default_writer_dir()")]
    writer_dir: Option<PathBuf>,
    #[builder(default = "false")]
    archive_persistence: bool,
    #[builder(default = "false")]
    writer_debug: bool,
    #[builder(default = "1")]
    crawl_depth: i32,
    #[builder(default = "5")]
    concurrent_browsers: i32,
    #[builder(default = "2")]
    url_retries: i32,
    #[builder(default = "self.default_base_url()")]
    base_url: String,
    #[builder(default = "self.default_archive_name()")]
    archive_name: Option<String>,
    #[builder(default = "false")]
    with_upload: bool,
    #[builder(default = "self.default_arweave_wallet_dir()")]
    arweave_key_dir: PathBuf,
    #[builder(default = "self.default_currency()")]
    currency: String,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        LaunchOptions {
            writer_port: Some(8080),
            writer_dir: Some(PathBuf::from(".")),
            crawl_depth: 1,
            concurrent_browsers: 5,
            url_retries: 2,
            base_url: BASE_URL.into(),
            archive_name: Some("archivoor".into()),
            with_upload: false,
            arweave_key_dir: PathBuf::from("res/test_wallet.json"),
            currency: "arweave".into(),
            archive_persistence: false,
            writer_debug: false,
        }
    }
}

impl LaunchOptions {
    pub fn default_builder() -> LaunchOptionsBuilder {
        LaunchOptionsBuilder::default()
    }
}

impl LaunchOptionsBuilder {
    fn default_archive_name(&self) -> Option<String> {
        Some(String::from("archivoor"))
    }
    fn default_base_url(&self) -> String {
        BASE_URL.into()
    }
    fn default_writer_dir(&self) -> Option<PathBuf> {
        Some(PathBuf::from(format!("")))
    }
    fn default_arweave_wallet_dir(&self) -> PathBuf {
        PathBuf::from("res/test_wallet.json")
    }
    fn default_currency(&self) -> String {
        String::from("arweave")
    }
}

impl Runner {
    pub async fn new(lo: LaunchOptions) -> anyhow::Result<Self> {
        let warc_writer = WarcWriter::new(
            lo.writer_port,
            lo.writer_dir.clone(),
            lo.archive_name.clone(),
            lo.archive_persistence,
            lo.writer_debug,
        )?;

        let uploader = if lo.with_upload {
            let u = Uploader::new(lo.arweave_key_dir.clone(), &lo.currency).await?;
            Some(u)
        } else {
            None
        };

        Ok(Runner {
            uploader,
            warc_writer,
            options: lo,
        })
    }

    pub async fn run(&self, url: &str) -> anyhow::Result<()> {
        let u = Url::from_str(url)?;
        let domain = match u.domain() {
            Some(d) => d,
            None => return Err(anyhow!("url must have a valid domain")),
        };
        let base_url = &format!("{}:{}", self.options.base_url, self.warc_writer.port());

        let full_url = &format!(
            "{}/{}/record/{}",
            base_url,
            self.warc_writer.archive_name(),
            url
        );

        let should_terminate = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate))?;
        signal_hook::flag::register(SIGINT, Arc::clone(&should_terminate))?;

        let (tx, rx) = sync_channel::<String>(1);

        info!(
            "Initializing crawl of {} with depth {}, {} browsers, {} retries.",
            url,
            self.options.crawl_depth,
            self.options.concurrent_browsers,
            self.options.url_retries
        );
        let mut crawler = Crawler::new(
            base_url,
            full_url,
            self.options.crawl_depth,
            self.options.concurrent_browsers,
            self.options.url_retries,
        );
        crawler.crawl(tx.clone(), should_terminate.clone()).await?;

        // we rename the files that the warc writer created for easy retrieval
        self.warc_writer
            .rename_files(domain, self.options.crawl_depth)?;

        if self.options.with_upload {
            match &self.uploader {
                Some(u) => {
                    if !should_terminate.load(Ordering::Relaxed) {
                        let id = u
                            .upload_latest_file(&self.warc_writer.archive_dir())
                            .await?;
                        println!(
                            "ids of the tx are \n File Tx: {} \n Metadata tx: {}",
                            id.0, id.1
                        );
                    }
                }
                None => {
                    error!("no uploader")
                }
            }
        }

        while !should_terminate.load(Ordering::Relaxed) {
            match rx.try_recv() {
                Ok(_res) => {
                    // when done, we read the recordings
                    break;
                }
                Err(TryRecvError::Empty) => {
                    sleep(Duration::from_secs(1));
                    continue;
                }
                Err(TryRecvError::Disconnected) => {
                    break;
                }
            }
        }

        Ok(())
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        debug!("{}", "Terminating runner...");
        self.warc_writer.terminate().unwrap();
        debug!("{}", "Child process killed, goodbye");
    }
}
