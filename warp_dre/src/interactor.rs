// interacts with a warp contract.
use anyhow::anyhow;
use arloader::{
    transaction::{Base64, FromUtf8Strs, Tag},
    Arweave,
};
use derive_builder::Builder;
use log::debug;
use reqwest::{Client, StatusCode, Url};
use std::{path::PathBuf, str::FromStr};

use crate::types::{InteractionResponse, APP_NAME, CONTRACT_TX_ID, INPUT, SDK, SMARTWEAVE_ACTION};

// example contract yS-CVbsg79p2sSrVAJZyRgE_d90BrxDjpAleRB-ZfXs
pub struct Interactor {
    client: Client,
    gateway_url: Url,
    arweave_key_path: PathBuf,
    contract_address: String,
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
    contract_address: String,
}

impl InteractorOptions {
    pub fn default_builder() -> InteractorOptionsBuilder {
        InteractorOptionsBuilder::default()
    }
}

impl InteractorOptionsBuilder {
    fn default_url(&self) -> Url {
        Url::from_str("https://d1o5nlqr4okus2.cloudfront.net/gateway").unwrap()
    }
    fn default_client(&self) -> Client {
        Client::new()
    }
    fn default_key_path(&self) -> PathBuf {
        PathBuf::from(".secrets/jwk.json")
    }
}

impl Interactor {
    pub fn new(lo: InteractorOptions) -> anyhow::Result<Self> {
        if !lo.arweave_key_path.exists() {
            return Err(anyhow!("arweave key path does not exist"));
        }
        if lo.contract_address == "" {
            return Err(anyhow!("contract address must be set"));
        }

        Ok(Self {
            client: lo.client,
            gateway_url: lo.url,
            arweave_key_path: lo.arweave_key_path,
            contract_address: lo.contract_address,
        })
    }

    pub async fn interact(&self, input: String) -> anyhow::Result<InteractionResponse> {
        let arweave = Arweave::from_keypair_path(
            PathBuf::from(self.arweave_key_path.clone()),
            Url::from_str("http://arweave.net").unwrap(),
        )
        .await
        .unwrap();

        let tx = arweave
            .create_transaction(vec![1], Some(self.create_tags(input)), None, (1, 1), false)
            .await?;

        let tx = arweave.sign_transaction(tx)?;

        // now we post to the client
        let res = self
            .client
            .post(format!(
                "{}/{}",
                self.gateway_url.clone(),
                "sequencer/register"
            ))
            .body(serde_json::to_string(&tx)?)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .send()
            .await?;

        if res.status() == StatusCode::OK {
            let res = res.json::<_>().await?;

            return Ok(res);
        } else {
            debug!("Status is {}", res.status());
            return Err(anyhow!(
                "Status:{}, error: {}",
                res.status(),
                res.text().await?
            ));
        }
    }

    fn create_tags(&self, input: String) -> Vec<Tag<Base64>> {
        vec![
            Tag::<Base64>::from_utf8_strs(APP_NAME, SMARTWEAVE_ACTION).unwrap(),
            Tag::<Base64>::from_utf8_strs("App-Version", "0.3.0").unwrap(),
            Tag::<Base64>::from_utf8_strs(SDK, "Warp").unwrap(),
            Tag::<Base64>::from_utf8_strs(CONTRACT_TX_ID, &self.contract_address).unwrap(),
            Tag::<Base64>::from_utf8_strs(INPUT, &input).unwrap(),
        ]
    }
}

// Interaction example https://arweave.app/tx/vJD6wxgynBgA4oDPaKPhEarKpQiw5ZMikv2-qUXCNtY
// https://sonar.warp.cc/#/app/interaction/2wXJx9r1_epUgWzyVXWVSS7XqsWZV7cKDCB_jUB7f-I
// https://github.com/warp-contracts/warp-dre-node
//https://github.com/warp-contracts/gateway/blob/main/src/gateway/router/gatewayRouter.ts
