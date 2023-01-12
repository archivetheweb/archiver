use derive_builder::Builder;
use reqwest::{Client, Url};
use std::{collections::HashMap, str::FromStr};

use crate::types::{BlacklistItem, Cached, ErrorsItem, Status};
pub struct WarpDRE {
    client: Client,
    url: Url,
}

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

    async fn get_status(&self) -> anyhow::Result<Status> {
        let res = self
            .client
            .get(format!("{}status", self.url))
            .send()
            .await?;

        let parsed = res.json::<Status>().await?;

        Ok(parsed)
    }

    async fn get_contract(&self, query: HashMap<String, String>) -> anyhow::Result<Status> {
        let res = self
            .client
            .get(format!("{}contract", self.url))
            .query(&query)
            .send()
            .await?;

        let parsed = res.json::<Status>().await?;

        Ok(parsed)
    }

    async fn get_cached(&self) -> anyhow::Result<Cached> {
        let res = self
            .client
            .get(format!("{}cached", self.url))
            .send()
            .await?;

        let parsed = res.json::<Cached>().await?;

        Ok(parsed)
    }

    async fn get_blacklist(&self) -> anyhow::Result<Vec<BlacklistItem>> {
        let res = self
            .client
            .get(format!("{}blacklist", self.url))
            .send()
            .await?;

        let parsed = res.json::<Vec<BlacklistItem>>().await?;

        Ok(parsed)
    }

    async fn get_errors(&self) -> anyhow::Result<Vec<ErrorsItem>> {
        let res = self
            .client
            .get(format!("{}errors", self.url))
            .send()
            .await?;

        let parsed = res.json::<Vec<ErrorsItem>>().await?;

        Ok(parsed)
    }
}

// #[cfg(test)]
// macro_rules! aw {
//     ($e:expr) => {
//         tokio_test::block_on($e)
//     };
// }

#[cfg(test)]
mod test {
    use super::*;
    use tokio_test::block_on;
    #[test]
    fn get_status() {}
}
