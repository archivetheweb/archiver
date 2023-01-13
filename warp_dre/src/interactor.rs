// interacts with a warp contract.
use anyhow::anyhow;
use derive_builder::Builder;
use reqwest::{Client, StatusCode, Url};
use std::{collections::HashMap, path::PathBuf, str::FromStr};

// example contract yS-CVbsg79p2sSrVAJZyRgE_d90BrxDjpAleRB-ZfXs
pub struct Interactor {
    client: Client,
    gateway_url: Url,
    arweave_key_path: PathBuf,
}

#[derive(Builder, Debug)]
#[builder(setter(into))]
pub struct InteractorOptions {
    #[builder(default = "self.default_url()")]
    url: Url,
    #[builder(default = "self.default_client()")]
    client: Client,
    #[builder(default = "self.default_key_path()")]
    arweave_key_path: PathBuf,
}

impl InteractorOptions {
    pub fn default_builder() -> InteractorOptionsBuilder {
        InteractorOptionsBuilder::default()
    }
}

impl InteractorOptionsBuilder {
    fn default_url(&self) -> Url {
        Url::from_str("https://d1o5nlqr4okus2.cloudfront.net").unwrap()
    }
    fn default_client(&self) -> Client {
        Client::new()
    }
    fn default_key_path(&self) -> PathBuf {
        PathBuf::from("./.secrets/jwk.json")
    }
}

impl Interactor {
    pub fn new(lo: InteractorOptions) -> anyhow::Result<Self> {
        if !lo.arweave_key_path.exists() {
            return Err(anyhow!("arweave key path does not exist"));
        }
        Ok(Self {
            client: lo.client,
            gateway_url: lo.url,
            arweave_key_path: lo.arweave_key_path,
        })
    }
}
