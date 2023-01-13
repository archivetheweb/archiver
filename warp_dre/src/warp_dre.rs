use derive_builder::Builder;
use reqwest::{Client, StatusCode, Url};
use std::{collections::HashMap, str::FromStr};

use crate::types::{BlacklistItem, Cached, ContractRoot, ContractWithQuery, ErrorsItem, Status};
pub struct WarpDRE {
    client: Client,
    url: Url,
}
use anyhow::anyhow;

#[derive(Builder, Debug)]
#[builder(setter(into))]
pub struct WarpDREOptions {
    #[builder(default = "self.default_url()")]
    url: Url,
    #[builder(default = "self.default_client()")]
    client: Client,
}

impl WarpDREOptions {
    pub fn default_builder() -> WarpDREOptionsBuilder {
        WarpDREOptionsBuilder::default()
    }
}

impl WarpDREOptionsBuilder {
    fn default_url(&self) -> Url {
        // https://dre-2.warp.cc/
        Url::from_str("https://dre-1.warp.cc").unwrap()
    }
    fn default_client(&self) -> Client {
        Client::new()
    }
}

impl WarpDRE {
    pub fn new(lo: WarpDREOptions) -> Self {
        WarpDRE {
            client: lo.client,
            url: lo.url,
        }
    }

    pub async fn get_status(&self) -> anyhow::Result<Status> {
        let res = self
            .client
            .get(format!("{}status", self.url))
            .send()
            .await?;

        if res.status() == StatusCode::OK {
            return Ok(res.json::<_>().await?);
        } else {
            return Err(anyhow!(res.text().await?));
        }
    }

    // TODO add state, validity, errorMessages, events and validation
    pub async fn get_contract(&self, contract_id: &str) -> anyhow::Result<ContractRoot> {
        let mut query: HashMap<String, String> = HashMap::new();
        query.insert("id".into(), contract_id.into());

        let res = self
            .client
            .get(format!("{}contract", self.url))
            .query(&query)
            .send()
            .await?;

        if res.status() == StatusCode::OK {
            return Ok(res.json::<_>().await?);
        } else {
            return Err(anyhow!(res.text().await?));
        }
    }

    pub async fn get_contract_with_query(
        &self,
        contract_id: &str,
        mut query: HashMap<String, String>,
    ) -> anyhow::Result<ContractWithQuery> {
        query.insert("id".into(), contract_id.into());

        let res = self
            .client
            .get(format!("{}contract", self.url))
            .query(&query)
            .send()
            .await?;

        if res.status() == StatusCode::OK {
            return Ok(res.json::<_>().await?);
        } else {
            return Err(anyhow!(res.text().await?));
        }
    }

    pub async fn get_cached(&self) -> anyhow::Result<Cached> {
        let res = self
            .client
            .get(format!("{}cached", self.url))
            .send()
            .await?;

        if res.status() == StatusCode::OK {
            return Ok(res.json::<_>().await?);
        } else {
            return Err(anyhow!(res.text().await?));
        }
    }

    pub async fn get_blacklist(&self) -> anyhow::Result<Vec<BlacklistItem>> {
        let res = self
            .client
            .get(format!("{}blacklist", self.url))
            .send()
            .await?;

        if res.status() == StatusCode::OK {
            return Ok(res.json::<_>().await?);
        } else {
            return Err(anyhow!(res.text().await?));
        }
    }

    pub async fn get_errors(&self) -> anyhow::Result<Vec<ErrorsItem>> {
        let res = self
            .client
            .get(format!("{}errors", self.url))
            .send()
            .await?;

        if res.status() == StatusCode::OK {
            return Ok(res.json::<_>().await?);
        } else {
            return Err(anyhow!(res.text().await?));
        }
    }
}
