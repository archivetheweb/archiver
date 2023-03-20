use std::collections::{BTreeMap, HashMap};

use anyhow::Context;
use arloader::Arweave;
use atw::{
    action::{DeleteArchiveRequest, RegisterUploader},
    state::{ArchiveRequest, ArchiveSubmission, State, Uploader},
};
use warp_dre::{
    interactor::{Interactor, InteractorOptionsBuilder},
    types::InteractionResponse,
    warp_dre::{WarpDRE, WarpDREOptionsBuilder},
};

pub struct Contract {
    contract_id: String,
    environment: String,
    interactor: Interactor,
    reader: WarpDRE,
}

const FUNCTION: &str = "function";

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

        let s = res.state.context("could not unwrap state").unwrap();
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

        let res = self
            .reader
            .get_contract_with_query(&self.contract_id, q)
            .await?;

        let s = res
            .result
            .context("could not unwrap archiving_request_for result")
            .unwrap();

        let s: Vec<ArchiveRequest> = serde_json::from_value(serde_json::Value::Array(s))?;

        Ok(s)
    }

    pub async fn uploaders(&self) -> anyhow::Result<HashMap<String, Uploader>> {
        let mut q = self.prepare_query();
        q.insert("query".into(), format!(r#"$.uploaders"#));

        let res = self
            .reader
            .get_contract_with_query(&self.contract_id, q)
            .await?;

        let s = res
            .result
            .context("could not unwrap uploaders result")
            .unwrap();

        let s =
            serde_json::from_value::<Vec<HashMap<String, Uploader>>>(serde_json::Value::Array(s))?
                .into_iter()
                .nth(0)
                .unwrap();

        Ok(s)
    }

    pub async fn archives_by_url(
        &self,
        url: &str,
        count: usize,
    ) -> anyhow::Result<Vec<ArchiveSubmission>> {
        let mut q = self.prepare_query();
        q.insert("query".into(), format!(r#"$.archives["{}"]"#, url));

        let res = self
            .reader
            .get_contract_with_query(&self.contract_id, q)
            .await?;

        let s = res
            .result
            .context("could not unwrap archives_by_url result")
            .unwrap();
        let s: Vec<BTreeMap<usize, ArchiveSubmission>> =
            serde_json::from_value(serde_json::Value::Array(s))?;

        let s = s.into_iter().nth(0).unwrap();

        let col = s.into_iter().rev().take(count).map(|x| x.1).collect();

        Ok(col)
    }

    pub async fn register_uploader(
        &self,
        uploader: RegisterUploader,
    ) -> anyhow::Result<InteractionResponse> {
        let mut v = serde_json::to_value(uploader)?;
        let t = v.as_object_mut().unwrap();
        t.insert(
            FUNCTION.into(),
            serde_json::Value::String("registerUploader".into()),
        );

        let res = self.interactor.interact(v).await?;

        Ok(res)
    }

    pub async fn submit_archive(
        &self,
        archive: &ArchiveSubmission,
    ) -> anyhow::Result<InteractionResponse> {
        let mut v = serde_json::to_value(&archive)?;
        let t = v.as_object_mut().unwrap();
        t.insert(
            FUNCTION.into(),
            serde_json::Value::String("submitArchive".into()),
        );

        let res = self.interactor.interact(v).await?;

        Ok(res)
    }

    pub async fn request_archiving(
        &self,
        archive: ArchiveRequest,
    ) -> anyhow::Result<InteractionResponse> {
        let mut v = serde_json::to_value(archive)?;
        let t = v.as_object_mut().unwrap();
        t.insert(
            FUNCTION.into(),
            serde_json::Value::String("requestArchiving".into()),
        );

        let res = self.interactor.interact(v).await?;

        Ok(res)
    }

    pub async fn delete_archive_request(
        &self,
        archive_id: &str,
    ) -> anyhow::Result<InteractionResponse> {
        let archive = DeleteArchiveRequest {
            archive_id: archive_id.into(),
        };
        let mut v = serde_json::to_value(archive)?;
        let t = v.as_object_mut().unwrap();
        t.insert(
            FUNCTION.into(),
            serde_json::Value::String("deleteArchiveRequest".into()),
        );

        let res = self.interactor.interact(v).await?;

        Ok(res)
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

    use atw::state::ArchiveOptions;
    use reqwest::Url;

    use crate::utils::{get_unix_timestamp, CONTRACT_ADDRESS};

    use super::*;

    const UPLOADER_ADDRESS: &str = "k-JI7M_U8hoezJuaaxKx8IBFJ3KRA9DGNCUw0Baltyc";

    #[test]
    fn test_state() {
        let arweave = tokio_test::block_on(Arweave::from_keypair_path(
            PathBuf::from(".secret/test_wallet.json"),
            Url::from_str("https://arweave.net").unwrap(),
        ))
        .unwrap();

        let c = Contract::new(&CONTRACT_ADDRESS, "mainnet", arweave).unwrap();

        let s = tokio_test::block_on(c.state()).unwrap();
        println!("{:#?}", s);
    }

    #[test]
    fn test_uploaders() {
        let arweave = tokio_test::block_on(Arweave::from_keypair_path(
            PathBuf::from(".secret/test_wallet.json"),
            Url::from_str("https://arweave.net").unwrap(),
        ))
        .unwrap();

        let c = Contract::new(&CONTRACT_ADDRESS, "mainnet", arweave).unwrap();

        let s = tokio_test::block_on(c.uploaders()).unwrap();
        println!("{:#?}", s);
    }

    #[test]
    fn test_requests_for() {
        let arweave = tokio_test::block_on(Arweave::from_keypair_path(
            PathBuf::from(".secret/test_wallet.json"),
            Url::from_str("https://arweave.net").unwrap(),
        ))
        .unwrap();

        let c = Contract::new(&CONTRACT_ADDRESS, "mainnet", arweave).unwrap();

        let s = tokio_test::block_on(c.archiving_requests_for(UPLOADER_ADDRESS)).unwrap();
        println!("{:#?}", s);
    }

    #[test]
    fn test_archives_by_url() {
        let arweave = tokio_test::block_on(Arweave::from_keypair_path(
            PathBuf::from(".secret/test_wallet.json"),
            Url::from_str("https://arweave.net").unwrap(),
        ))
        .unwrap();

        let c = Contract::new(&CONTRACT_ADDRESS, "mainnet", arweave).unwrap();

        let s = tokio_test::block_on(c.archives_by_url("example.com", 10)).unwrap();
        println!("{:#?}", s);

        assert!(s.len() > 0);
    }

    #[test]
    fn test_register_uploader() {
        let arweave = tokio_test::block_on(Arweave::from_keypair_path(
            PathBuf::from(".secret/test_wallet.json"),
            Url::from_str("https://arweave.net").unwrap(),
        ))
        .unwrap();

        let c = Contract::new(&CONTRACT_ADDRESS, "mainnet", arweave).unwrap();

        let uploader = RegisterUploader {
            friendly_name: "alice".into(),
        };

        let s = tokio_test::block_on(c.register_uploader(uploader)).unwrap();
        println!("{:#?}", s);
    }

    #[test]
    fn test_delete_archive_request() {
        let arweave = tokio_test::block_on(Arweave::from_keypair_path(
            PathBuf::from(".secret/test_wallet.json"),
            Url::from_str("https://arweave.net").unwrap(),
        ))
        .unwrap();

        let c = Contract::new(&CONTRACT_ADDRESS, "mainnet", arweave).unwrap();

        let s = tokio_test::block_on(
            c.delete_archive_request("ol2dKXgntbxj5PFtbWvgmftCLibrqkjIrraQYzcweFU".into()),
        )
        .unwrap();
        println!("{:#?}", s);
    }

    #[test]
    fn test_submit_archives() {
        let arweave = tokio_test::block_on(Arweave::from_keypair_path(
            PathBuf::from(".secret/test_wallet.json"),
            Url::from_str("https://arweave.net").unwrap(),
        ))
        .unwrap();

        let c = Contract::new(&CONTRACT_ADDRESS, "mainnet", arweave).unwrap();

        let archive = ArchiveSubmission {
            full_url: "https://example.com?hi".into(),
            arweave_tx: "aa".into(),
            size: 1,
            uploader_address: UPLOADER_ADDRESS.into(),
            archive_request_id: "ol2dKXgntbxj5PFtbWvgmftCLibrqkjIrraQYzcweFU".into(),
            timestamp: get_unix_timestamp().as_secs() as i64,
            options: ArchiveOptions {
                depth: 0,
                crawl_type: atw::state::CrawlType::DomainAndLinks,
                domain_only: None,
            },
            screenshot_tx: "".into(),
            title: "".into(),
        };

        let s = tokio_test::block_on(c.submit_archive(&archive)).unwrap();
        println!("{:#?}", s);
    }
}
