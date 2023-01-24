// use crate::

use std::collections::HashMap;

use anyhow::anyhow;
use arloader::Arweave;
use atw::state::{ArchiveRequest, State};
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
    pub fn new(contract_id: &str, environment: &str, arweave: Arweave) -> anyhow::Result<Self> {
        let interactor = Interactor::new(
            InteractorOptionsBuilder::default()
                .contract_address(contract_id)
                .build()?,
            arweave,
        )?;

        let reader = WarpDRE::new(WarpDREOptionsBuilder::default().build()?);

        return Ok(Contract {
            contract_id: contract_id.into(),
            environment: environment.into(),
            interactor,
            reader,
        });
    }

    pub async fn state(&self) -> anyhow::Result<State> {
        let q = self.prepare_query();
        let res = self
            .reader
            .get_contract_with_query(&self.contract_id, q)
            .await?;

        let s = res.state.unwrap();
        let s: State = serde_json::from_value(s)?;

        Ok(s)
    }

    pub async fn archiving_requests_for(
        &self,
        address: &str,
    ) -> anyhow::Result<Vec<ArchiveRequest>> {
        let mut q = self.prepare_query();
        q.insert(
            "query".into(),
            format!(r#"$.archiveRequests.[?(@.uploaderAddress=="{}")]"#, address),
        );

        println!("{:?}", q);
        let res = self
            .reader
            .get_contract_with_query(&self.contract_id, q)
            .await?;

        let s = match res.result {
            Some(s) => s,
            None => return Err(anyhow!("Could not unwrap result")),
        };

        let s: Vec<ArchiveRequest> = serde_json::from_value(serde_json::Value::Array(s))?;

        Ok(s)
    }

    fn prepare_query(&self) -> HashMap<String, String> {
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
    fn test_state() {
        let arweave = tokio_test::block_on(Arweave::from_keypair_path(
            PathBuf::from("res/test_wallet.json"),
            Url::from_str("https://arweave.net").unwrap(),
        ))
        .unwrap();

        let c = Contract::new(
            "WT4rx8FwvzHLqgeaJsxK72rotZFOo3E9qSJ_3WSNO7U".into(),
            "mainnet",
            arweave,
        )
        .unwrap();

        let s = tokio_test::block_on(c.state()).unwrap();
        println!("{:#?}", s);
    }

    #[test]
    fn test_requests_for() {
        let arweave = tokio_test::block_on(Arweave::from_keypair_path(
            PathBuf::from("res/test_wallet.json"),
            Url::from_str("https://arweave.net").unwrap(),
        ))
        .unwrap();

        let c = Contract::new(
            "WT4rx8FwvzHLqgeaJsxK72rotZFOo3E9qSJ_3WSNO7U".into(),
            "mainnet",
            arweave,
        )
        .unwrap();

        let s = tokio_test::block_on(
            c.archiving_requests_for("H9hkQ6njDcDNbP7thTDmgprMZP_5QJGMxyJAwbyBAGg"),
        )
        .unwrap();
        println!("{:#?}", s);

        assert!(s.len() > 0);
    }
}
