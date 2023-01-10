use std::{
    fs,
    path::PathBuf,
    process::{self, Command},
    sync::{
        mpsc::{sync_channel, TryRecvError},
        Arc,
        {atomic::AtomicBool, atomic::Ordering},
    },
    thread::sleep,
    time::Duration,
};

use signal_hook::consts::{SIGINT, SIGTERM};

use crate::{
    crawler::Crawler,
    uploader::Uploader,
    utils::{ARCHIVE_DIR, BASE_DIR, BASE_URL},
    warc_writer::WarcWriter,
};

pub struct Runner {
    uploader: Option<Uploader>,
    warc_writer: WarcWriter,
    options: LaunchOptions,
}

#[derive(Builder, Debug)]
#[builder(setter(into))]
pub struct LaunchOptions {
    #[builder(default = "8080")]
    writer_port: u16,
    #[builder(default = "self.default_writer_dir()")]
    writer_dir: PathBuf,
    #[builder(default = "1")]
    crawl_depth: i32,
    #[builder(default = "5")]
    concurrent_browsers: i32,
    #[builder(default = "2")]
    url_retries: i32,
    #[builder(default = "self.default_base_url()")]
    base_url: String,
    #[builder(default = "self.default_archive_name()")]
    archive_name: String,
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
            writer_port: 8080,
            writer_dir: PathBuf::from("."),
            crawl_depth: 1,
            concurrent_browsers: 5,
            url_retries: 2,
            base_url: BASE_URL.into(),
            archive_name: "archivoor".into(),
            with_upload: false,
            arweave_key_dir: PathBuf::from("res/test_wallet.json"),
            currency: "arweave".into(),
        }
    }
}

impl LaunchOptionsBuilder {
    fn default_archive_name(&self) -> String {
        String::from("archivoor")
    }
    fn default_base_url(&self) -> String {
        BASE_URL.into()
    }
    fn default_writer_dir(&self) -> PathBuf {
        PathBuf::from(format!("./{}/{}", BASE_DIR, ARCHIVE_DIR))
    }
    fn default_arweave_wallet_dir(&self) -> PathBuf {
        PathBuf::from("res/test_wallet.json")
    }
    fn default_currency(&self) -> String {
        String::from("arweave")
    }
}

impl Runner {
    pub async fn new(launch_options: LaunchOptions) -> anyhow::Result<Self> {
        setup_dir(&launch_options.writer_dir)?;

        let writer_port = launch_options.writer_port;
        let warc_writer = WarcWriter::new(writer_port, true)?;

        let uploader = if launch_options.with_upload {
            let u = Uploader::new(
                launch_options.arweave_key_dir.clone(),
                &launch_options.currency,
            )
            .await?;
            Some(u)
        } else {
            None
        };

        Ok(Runner {
            uploader,
            warc_writer,
            options: launch_options,
        })
    }

    pub async fn run(mut self, url: &str) -> anyhow::Result<()> {
        let url = format!(
            "{}:{}/{}/record/{}",
            self.options.base_url, self.options.writer_port, self.options.archive_name, url
        );

        let should_terminate = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate))?;
        signal_hook::flag::register(SIGINT, Arc::clone(&should_terminate))?;

        let (tx, rx) = sync_channel::<String>(1);

        let mut crawler = Crawler::new(
            &url,
            self.options.crawl_depth,
            self.options.concurrent_browsers,
            self.options.url_retries,
        );
        crawler.crawl(tx.clone(), should_terminate.clone()).await?;

        if self.options.with_upload {
            match self.uploader {
                Some(u) => {
                    if !should_terminate.load(Ordering::Relaxed) {
                        let id = u.upload_latest().await?;
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

        debug!("{}", "Terminating...");
        self.warc_writer.terminate()?;
        debug!("{}", "Child process killed, goodbye");

        Ok(())
    }
}

fn setup_dir(work_dir: &PathBuf) -> anyhow::Result<()> {
    // first check if we have a collection with wb-manager
    if let Err(_) = fs::read_dir(work_dir) {
        let res = Command::new("wb-manager")
            .args(["init", ARCHIVE_DIR])
            .status()?;

        if !res.success() {
            process::exit(res.code().unwrap());
        }
        let mut new_dir = work_dir.clone();
        new_dir.push("screenshots");

        fs::create_dir(new_dir)?;
    }
    Ok(())
}
