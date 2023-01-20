// use crate::

use std::collections::HashMap;

use arloader::Arweave;
use atw::state::State;
use warp_dre::{
    interactor::{Interactor, InteractorOptionsBuilder},
    warp_dre::{WarpDRE, WarpDREOptionsBuilder},
};

pub struct Contract {
    contract_id: String,
    environment: String,
    interactor: Interactor,
    reader: WarpDRE,
}

impl Contract {
    pub fn new(contract_id: String, environment: String, arweave: Arweave) -> anyhow::Result<Self> {
        let interactor = Interactor::new(
            InteractorOptionsBuilder::default()
                .contract_address(&contract_id)
                .build()?,
            arweave,
        )?;

        let reader = WarpDRE::new(WarpDREOptionsBuilder::default().build()?);

        return Ok(Contract {
            contract_id,
            environment,
            interactor,
            reader,
        });
    }

    pub async fn state(&self) -> anyhow::Result<State> {
        let q = self.get_query_container();
        let res = self
            .reader
            .get_contract_with_query(&self.contract_id, q)
            .await?;

        let s = res.state.unwrap();
        let s: State = serde_json::from_value(s)?;

        Ok(s)
    }

    pub fn get_query_container(&self) -> HashMap<String, String> {
        let mut q = HashMap::new();
        match self.environment.as_str() {
            "testnet" => {
                q.insert("network".to_owned(), "testnet".to_owned());
            }
            _ => {}
        }

        q
    }
}

#[cfg(test)]
mod test {

    use std::{path::PathBuf, str::FromStr};

    use reqwest::Url;

    use super::*;
    #[test]
    fn create() {
        let arweave = tokio_test::block_on(Arweave::from_keypair_path(
            PathBuf::from("res/test_wallet.json"),
            Url::from_str("https://arweave.net").unwrap(),
        ))
        .unwrap();

        let c = Contract::new(
            "8iOzf88NnWPk2h45QsqRhtKm0wM1z_a97O2oKgTfOio".into(),
            "testnet".into(),
            arweave,
        )
        .unwrap();

        let s = tokio_test::block_on(c.state());
        println!("{:#?}", s);
    }
}
