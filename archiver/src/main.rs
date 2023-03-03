use std::{
    path::PathBuf,
    str::FromStr,
    sync::{atomic::AtomicBool, Arc},
};

use anyhow::{anyhow, Context};
use archiver::{
    archiver::Archiver,
    contract::Contract,
    types::BundlrBalance,
    utils::{BUNDLR_URL, CONTRACT_ADDRESS},
};
use arloader::Arweave;
use log::debug;
use reqwest::Url;
use signal_hook::consts::{SIGINT, SIGTERM};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    debug!("{}", "In debug mode");

    let should_terminate = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate))?;
    signal_hook::flag::register(SIGINT, Arc::clone(&should_terminate))?;
    let path = PathBuf::from(".secret/wallet.json");
    let p = path.clone();
    let arweave = Arweave::from_keypair_path(path, Url::from_str("https://arweave.net")?)
        .await
        .context(format!("could not open arweave wallet from path {:?}", p))?;

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
        return Err(anyhow!("Not registered as an uploader"));
    }

    let mut archiver = Archiver::new(3);

    archiver
        .archive(Arc::new(contract), wallet_address, should_terminate)
        .await?;

    Ok(())
}
