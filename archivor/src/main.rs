use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::anyhow;
use archivoor_v1::{
    contract::Contract,
    runner::{LaunchOptions, Runner},
    types::{BundlrBalance, BUNDLR_URL},
    utils::{get_unix_timestamp, CONTRACT_ADDRESS},
};
use arloader::Arweave;
use atw::state::{ArchiveOptions, ArchiveRequest, ArchiveSubmission};
use chrono::{DateTime, NaiveDateTime, Utc};
use cron::Schedule;
use futures::StreamExt;
use log::{debug, error};
use reqwest::Url;
use signal_hook::consts::{SIGINT, SIGTERM};
use tokio::{sync::mpsc, sync::mpsc::Sender, time::sleep};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    debug!("{}", "In debug mode");

    let should_terminate = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate))?;
    signal_hook::flag::register(SIGINT, Arc::clone(&should_terminate))?;

    let arweave = Arweave::from_keypair_path(
        PathBuf::from(".secret/test_wallet.json"),
        Url::from_str("https://arweave.net")?,
    )
    .await?;

    let wallet_address = arweave.crypto.wallet_address()?.to_string();

    debug!("Arweave Wallet {} loaded", wallet_address);

    // check if we have funds in bundlr
    let res = match reqwest::get(format!(
        "{}/account/balance/arweave?address={}",
        BUNDLR_URL, &wallet_address
    ))
    .await
    {
        Ok(res) => res,
        Err(e) => return Err(anyhow!(e.to_string())),
    };
    let res = res.json::<BundlrBalance>().await?;

    if res.balance == "0" {
        return Err(anyhow!("no funds in bundlr address {} ", &wallet_address));
    }

    let contract = Contract::new(&CONTRACT_ADDRESS, "mainnet", arweave)?;

    let uploaders = contract.uploaders().await?;

    // we ensure we are an uploader
    if !uploaders.contains_key(&wallet_address) {
        return Err(anyhow!("Not registered as an uploader"));
    }

    let mut archiver = Archiver::new(3);

    archiver
        .archive(Arc::new(contract), wallet_address, should_terminate)
        .await?;

    Ok(())
}

pub struct Archiver {
    processing: HashSet<String>,
    concurrent_workers: i32,
}

impl Archiver {
    pub fn new(concurrent_workers: i32) -> Self {
        Archiver {
            processing: HashSet::new(),
            concurrent_workers: concurrent_workers,
        }
    }
    pub async fn archive(
        &mut self,
        contract: Arc<Contract>,
        wallet_address: String,
        should_terminate: Arc<AtomicBool>,
    ) -> anyhow::Result<()> {
        let (work_fetcher_tx, mut work_fetcher_rx) = mpsc::channel::<ArchiveRequest>(100);
        let (processed_archive_tx, mut processed_archive_rx) = mpsc::channel::<ArchiveRequest>(100);
        let (archiver_tx, archiver_rx) = mpsc::channel::<ArchiveRequest>(100);

        let ct = contract.clone();
        let wa = wallet_address.clone();
        let st = should_terminate.clone();

        // create a thread where we fetch new work
        tokio::spawn(async move {
            loop {
                if should_terminate.load(Ordering::Relaxed) {
                    return;
                }
                match Self::fetch_new_work(
                    contract.clone(),
                    wallet_address.clone(),
                    work_fetcher_tx.clone(),
                    should_terminate.clone(),
                )
                .await
                {
                    Ok(_) => {
                        debug!("new work fetched");
                    }
                    Err(e) => {
                        error!("error fetching new work {}", e)
                    }
                };
                if should_terminate.load(Ordering::Relaxed) {
                    return;
                }
                let timeout = 30;
                debug!("sleeping for {} seconds", timeout);
                sleep(Duration::from_secs(timeout)).await;
            }
        });

        self.processor(ct, wa, st.clone(), archiver_rx, processed_archive_tx);

        while !st.load(Ordering::Relaxed) {
            let res = work_fetcher_rx.try_recv();

            if res.is_ok() {
                let archive_request = res.unwrap();

                if self.processing.contains(&archive_request.id) {
                    // we do nothing, it's already processing
                } else {
                    debug!("Sending new archive {:?}", archive_request);
                    self.processing.insert(archive_request.id.clone());
                    archiver_tx.send(archive_request).await?;
                }
            } else {
                // we check if channel is empty
                match res.err().unwrap() {
                    mpsc::error::TryRecvError::Empty => {}
                    mpsc::error::TryRecvError::Disconnected => debug!("disconnected"),
                }
            }

            match processed_archive_rx.try_recv() {
                Ok(req) => {
                    self.processing.remove(&req.id);
                    debug!("Processed archive request with id: {}", req.id);
                }
                Err(_) => {}
            }

            sleep(Duration::from_millis(10)).await;
        }
        Ok(())
    }

    fn processor(
        &self,
        contract: Arc<Contract>,
        wallet_address: String,
        should_terminate: Arc<AtomicBool>,
        archiver_rx: mpsc::Receiver<ArchiveRequest>,
        processed_archiver_tx: mpsc::Sender<ArchiveRequest>,
    ) {
        let concurrency = self.concurrent_workers.clone();

        tokio::spawn(async move {
            tokio_stream::wrappers::ReceiverStream::new(archiver_rx)
                .for_each_concurrent(concurrency as usize, |req| {
                    let should_terminate = should_terminate.clone();
                    let c = contract.clone();
                    let w = wallet_address.clone();
                    let tx = processed_archiver_tx.clone();
                    async move {
                        let id = req.id.clone();
                        debug!("Archive running for req {:#?}", req);
                        let res = Self::run(c, w, &req, should_terminate).await;
                        debug!("{:?}", res);
                        match res {
                            Ok(_) => {
                                match tx.send(req).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("Could not send archive processed {:?}", e)
                                    }
                                };
                            }
                            Err(e) => {
                                error!("Error archiving req {:?}. Error: {:?}", id, e);
                            }
                        };
                        return;
                    }
                })
                .await;

            return;
        });
    }

    async fn fetch_new_work(
        c: Arc<Contract>,
        wallet_address: String,
        archiver_tx: Sender<ArchiveRequest>,
        should_terminate: Arc<AtomicBool>,
    ) -> anyhow::Result<()> {
        if should_terminate.load(Ordering::Relaxed) {
            return Err(anyhow!("Early termination"));
        }

        let requests = c.archiving_requests_for(&wallet_address).await?;
        debug!("Requests: {:#?}", requests);

        let mut valid_reqs = vec![];

        // we loop through the request, if one of them is expired, we delete it
        for r in requests {
            if r.end_timestamp < get_unix_timestamp().as_secs() as i64 {
                debug!("deleting archive request with id {}", r.id);
                c.delete_archive_request(&r.id).await?;
                continue;
            }
            valid_reqs.push(r);
        }

        for req in valid_reqs {
            let schedule = match Schedule::from_str(&req.frequency) {
                Ok(sched) => sched,
                Err(e) => {
                    error!("invalid schedule for request {:?}, error: {}", req, e);
                    continue;
                }
            };

            let after = DateTime::<Utc>::from_utc(
                NaiveDateTime::from_timestamp_opt(
                    req.latest_archived_timestamp.try_into().unwrap_or(0),
                    0,
                )
                .unwrap(),
                Utc,
            );

            let mut schedule_iter = schedule.after(&after);

            match schedule_iter.nth(0) {
                Some(next_time) => {
                    if next_time.cmp(&Utc::now()) == std::cmp::Ordering::Greater {
                        continue;
                    }
                }
                None => continue,
            };

            archiver_tx.send(req).await?;
        }
        Ok(())
    }

    async fn run(
        contract: Arc<Contract>,
        wallet_address: String,
        req: &ArchiveRequest,
        should_terminate: Arc<AtomicBool>,
    ) -> anyhow::Result<()> {
        let options = LaunchOptions::default_builder()
            .with_upload(true)
            .writer_dir(Some("/tmp/".into()))
            .writer_port(None)
            .writer_debug(false)
            .archive_name(None)
            .crawl_depth(req.options.depth)
            // .domain_only(req.options.domain_only)
            .concurrent_browsers(10)
            .build()?;

        debug!("Launching crawler with options: \n {:#?}", options);

        let r = Runner::new(options).await?;

        if should_terminate.load(Ordering::Relaxed) {
            return Err(anyhow!("Early termination"));
        }

        let url = &req.options.urls[0];

        let result = r.run_archiving(url).await?;
        let title = result.title.clone();
        debug!("result {:?}", result);

        if should_terminate.load(Ordering::Relaxed) {
            return Err(anyhow!("Early termination"));
        }

        let main_file = result.warc_files[0].clone();

        let metadata = fs::metadata(&main_file)?;

        let size = metadata.len();

        debug!("{:#?}  {:#?}", &result.archive_info, size);

        let ts = result.archive_info.unix_ts();

        let upload_result = r.run_upload_crawl(result).await?;

        debug!("Upload result {:#?}", upload_result);

        if should_terminate.load(Ordering::Relaxed) {
            return Err(anyhow!("Early termination"));
        }

        contract
            .submit_archive(ArchiveSubmission {
                full_url: url.into(),
                size: size as usize,
                uploader_address: wallet_address.clone(),
                archive_request_id: req.id.clone(),
                timestamp: ts,
                arweave_tx: upload_result.warc_id[0].clone(),
                options: ArchiveOptions {
                    depth: req.options.depth,
                    domain_only: req.options.domain_only,
                },
                screenshot_tx: upload_result.screenshot_id,
                title: title,
            })
            .await?;
        Ok(())
    }
}
