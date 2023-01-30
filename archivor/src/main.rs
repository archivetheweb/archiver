use std::{
    fs,
    path::PathBuf,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::sleep,
    time::Duration,
};

use anyhow::anyhow;
use archivoor_v1::{
    contract::Contract,
    runner::{LaunchOptions, Runner},
    utils::{get_unix_timestamp, ArchiveInfo},
};
use arloader::Arweave;
use atw::state::{ArchiveOptions, ArchiveSubmission};
use chrono::{DateTime, NaiveDateTime, Utc};
use cron::Schedule;
use log::{debug, error};
use reqwest::Url;
use signal_hook::consts::{SIGINT, SIGTERM};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    debug!("{}", "In debug mode");

    let should_terminate = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate))?;
    signal_hook::flag::register(SIGINT, Arc::clone(&should_terminate))?;

    let arweave = Arweave::from_keypair_path(
        PathBuf::from("res/test_wallet.json"),
        Url::from_str("https://arweave.net")?,
    )
    .await?;

    let wallet_address = arweave.crypto.wallet_address()?.to_string();

    let c = Contract::new(
        "YQLMJqrN8jTAmuEB_nWqgK9cKT72VwtvTqv7KP7ZOUc".into(),
        "mainnet",
        arweave,
    )?;

    let uploaders = c.uploaders().await?;

    // we ensure we are an uploader
    if !uploaders.contains_key(&wallet_address) {
        return Err(anyhow!("Not registered as an uploader"));
    }

    loop {
        if should_terminate.load(Ordering::Relaxed) {
            return Ok(());
        }
        match run(&c, wallet_address.clone()).await {
            Ok(_) => {}
            Err(e) => {
                error!("Error in main loop: {}", e)
            }
        };
        if should_terminate.load(Ordering::Relaxed) {
            return Ok(());
        }
        debug!("sleeping for 10 seconds");
        sleep(Duration::from_secs(10));
    }
}

async fn run(c: &Contract, wallet_address: String) -> anyhow::Result<()> {
    let requests = c.archiving_requests_for(&wallet_address).await?;
    debug!("Requests: {:#?}", requests);

    let mut valid_reqs = vec![];

    // we loop through the request, if one of them is expired, we delete it
    for r in requests {
        if r.end_timestamp < get_unix_timestamp().as_secs() as usize {
            debug!("deleting archive request with id {}", r.id);
            c.delete_archive_request(&r.id).await?;
            continue;
        }
        valid_reqs.push(r);
    }

    debug!("Valid reqs: {:#?}", valid_reqs);

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
                req.latest_upload_timestamp.try_into().unwrap_or(0),
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

        debug!("running for request {:?} ", req);

        let options = LaunchOptions::default_builder()
            .with_upload(false)
            .writer_dir(Some(".".into()))
            .writer_port(None)
            .writer_debug(false)
            .archive_name(None)
            .crawl_depth(req.crawl_options.depth)
            // todo
            // .domain_only(req.crawl_options.domain_only)
            .concurrent_browsers(10)
            .build()?;

        debug!("Launching crawler with options: \n {:#?}", options);

        let r = Runner::new(options).await?;

        let url = &req.crawl_options.urls[0];

        let filenames = r.run_crawl(url).await?;
        debug!("filenames {:?}", filenames);

        let main_file = filenames[0].clone();

        let metadata = fs::metadata(&main_file)?;

        let size = metadata.len();

        let info = ArchiveInfo::new(&main_file)?;

        debug!("{:?}  {:?}", info, size);

        let tx_ids = r.run_upload_files(filenames).await?;

        debug!("tx_ids {:?}", tx_ids);

        c.submit_archive(ArchiveSubmission {
            full_url: url.into(),
            size: size as usize,
            uploader_address: wallet_address.clone(),
            archive_request_id: req.id,
            timestamp: info.unix_ts() as usize,
            arweave_tx: tx_ids[0].clone(),
            options: ArchiveOptions {
                depth: req.crawl_options.depth,
                domain_only: req.crawl_options.domain_only,
            },
        })
        .await?;
    }
    Ok(())
}
