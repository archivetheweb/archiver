use anyhow::Context;
use arloader::{
    transaction::{Base64, FromUtf8Strs, Tag},
    Arweave,
};
use derive_builder::Builder;
use log::debug;
use reqwest::{Client, StatusCode, Url};
use serde_json::Value;
use std::str::FromStr;

use crate::{
    errors::WarpDREError,
    types::{InteractionResponse, APP_NAME, CONTRACT_TX_ID, INPUT, SDK, SMARTWEAVE_ACTION},
};

pub struct Interactor {
    client: Client,
    gateway_url: Url,
    contract_address: String,
    pub arweave: Arweave,
}

#[derive(Builder)]
#[builder(setter(into))]
pub struct InteractorOptions {
    #[builder(default = "self.default_url()")]
    url: Url,
    #[builder(default = "self.default_client()")]
    client: Client,
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
}

impl Interactor {
    pub fn new(lo: InteractorOptions, arweave: Arweave) -> anyhow::Result<Self> {
        if lo.contract_address == "" {
            return Err(WarpDREError::ArgumentError("contract address must be set".into()).into());
        }

        Ok(Self {
            client: lo.client,
            gateway_url: lo.url,
            contract_address: lo.contract_address,
            arweave,
        })
    }

    pub async fn interact(&self, input: Value) -> anyhow::Result<InteractionResponse> {
        let tx = self
            .arweave
            .create_transaction(
                vec![1],
                Some(self.create_tags(input.to_string())),
                None,
                (1, 1),
                false,
            )
            .await
            .context(format!(
                "could not create arweave stransaction with input {:?}",
                input
            ))?;

        let tx = self.arweave.sign_transaction(tx)?;

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
            .await
            .context(format!("could not create register with warp sequencer"))?;

        if res.status() == StatusCode::OK {
            let res = res.json::<_>().await?;

            return Ok(res);
        } else {
            debug!("Status is {}", res.status());
            return Err(WarpDREError::WarpGatewayError(format!(
                "Status:{}, error: {}",
                res.status(),
                res.text().await?
            ))
            .into());
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
