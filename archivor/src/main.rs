use std::{cmp::Ordering, path::PathBuf, str::FromStr};

use anyhow::anyhow;
use archivoor_v1::{
    contract::Contract,
    runner::{LaunchOptions, Runner},
    utils::get_unix_timestamp,
};
use arloader::Arweave;
use chrono::{DateTime, NaiveDateTime, Utc};
use cron::Schedule;
use log::debug;
use reqwest::Url;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    debug!("{}", "In debug mode");

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
        // TODO validate frequency

        let schedule = Schedule::from_str(&req.frequency).unwrap();

        let after = DateTime::<Utc>::from_utc(
            NaiveDateTime::from_timestamp_opt(req.latest_upload_timestamp.try_into().unwrap(), 0)
                .unwrap(),
            Utc,
        );

        let mut schedule_iter = schedule.after(&after);

        match schedule_iter.nth(0) {
            Some(next_time) => {
                if next_time.cmp(&Utc::now()) == Ordering::Greater {
                    continue;
                }
            }
            None => continue,
        };

        println!("running!");

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

        println!("filenames {:?}", filenames);

        let tx_ids = r.run_upload_files(filenames).await?;

        println!("tx_ids {:?}", tx_ids);

        // TODO add concurrency
    }
    Ok(())
}
