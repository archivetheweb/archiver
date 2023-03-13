use std::{
    ops::Div,
    path::PathBuf,
    str::FromStr,
    sync::{atomic::AtomicBool, Arc},
};

use anyhow::{anyhow, Context};
use archiver::{
    archiver::{Archiver, ArchiverOptionsBuilder},
    contract::Contract,
    types::BundlrBalance,
    utils::{BUNDLR_URL, CONTRACT_ADDRESS},
};
use arloader::Arweave;
use clap::Parser;
use log::debug;
use reqwest::Url;
use signal_hook::consts::{SIGINT, SIGTERM};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Archive The Web Uploader CLI", long_about = None)]
struct Args {
    /// Total number of concurrent crawls
    #[arg(short = 'c', long, default_value_t = 3)]
    concurrent_crawlers: u8,
    /// Total number of concurrent tabs open within a crawl
    #[arg(short = 't', long, default_value_t = 10)]
    concurrent_tabs: u8,
    /// Number of retries per failed URL
    #[arg(short = 'r', long, default_value_t = 2)]
    retries: u8,
    /// Whether to upload the crawls or not
    #[arg(short = 'u', long, default_value_t = true)]
    with_upload: bool,
    /// Minimum time in seconds to wait after a tab navigates to a page
    #[arg(long, default_value_t = 5)]
    min_wait_after_navigation: u64,
    /// Maximum time in seconds to wait after a tab navigates to a page
    #[arg(long, default_value_t = 7)]
    max_wait_after_navigation: u64,
    /// Maximum time the browser will wait for an event before timing out
    #[arg(long, default_value_t = 45)]
    browser_timeout: u64,
    /// Maximum time in seconds to wait after a tab navigates to a page
    #[arg(short = 'd', long)]
    writer_directory: Option<PathBuf>,
    /// Frequency of fetching for new archive requests in seconds
    #[arg(short = 'f', long, default_value_t = 30)]
    fetching_frequency: u8,
    /// Maximum time in seconds to wait after a tab navigates to a page
    #[arg(short = 'b', long)]
    balance: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Args::parse();

    match std::env::var("RUST_LOG") {
        Ok(env) => {
            if env == "debug" {
                println!("{number:/>width$}", number = "", width = 20);
                println!("{}", "Debug mode enabled");
                println!("{number:/>width$}", number = "", width = 20);
                println!();
            }
        }
        _ => {}
    }

    let path = PathBuf::from(".secret/wallet.json");
    let arweave = Arweave::from_keypair_path(path.clone(), Url::from_str("https://arweave.net")?)
        .await
        .context(format!(
            "could not open arweave wallet from path {:?}",
            path
        ))?;

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
    if args.balance {
        let b = match res.balance.parse::<f64>() {
            Ok(num) => num,
            Err(e) => panic!("Couldn't parse balance {}", e),
        };

        println!(
            "balance: {} winston or {:.12} AR for address {}",
            res.balance,
            b.div(1000000000000.0),
            &wallet_address,
        );
        return Ok(());
    }

    let environment = "mainnet";
    let contract = Contract::new(&CONTRACT_ADDRESS, environment, arweave).context(format!(
        "could not initiate contract with address {} on env {}",
        CONTRACT_ADDRESS.as_str(),
        environment
    ))?;

    let uploaders = contract
        .uploaders()
        .await
        .context("could not fetch uploaders")?;

    // we ensure we are an uploader
    if !uploaders.contains_key(&wallet_address) {
        return Err(anyhow!(
            "{} is not registered as an uploader",
            wallet_address
        ));
    }

    debug!("Starting Uploader with {:#?}", args.clone());

    let archive_options = ArchiverOptionsBuilder::default_builder()
        .writer_dir(args.writer_directory)
        .concurrent_crawlers(args.concurrent_crawlers)
        .concurrent_tabs(args.concurrent_tabs)
        .fetch_frequency(args.fetching_frequency)
        .url_retries(args.retries)
        .with_upload(args.with_upload)
        .browser_timeout(args.browser_timeout)
        .min_wait_after_navigation(args.min_wait_after_navigation)
        .max_wait_after_navigation(args.max_wait_after_navigation)
        .build()?;

    let mut archiver = Archiver::new(archive_options);

    let should_terminate = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate))?;
    signal_hook::flag::register(SIGINT, Arc::clone(&should_terminate))?;

    archiver
        .archive(Arc::new(contract), wallet_address, should_terminate)
        .await?;

    Ok(())
}
