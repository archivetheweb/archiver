use std::collections::HashMap;

use ::warp_dre::{
    interactor::{self, InteractorOptionsBuilder},
    warp_dre::WarpDREOptionsBuilder,
};
use warp_dre::warp_dre;

macro_rules! aw {
    ($e:expr) => {
        tokio_test::block_on($e)
    };
}

// cargo test -- --include-ignored

#[test]
#[ignore = "outbound_calls"]
fn get_cache() -> anyhow::Result<()> {
    let client = warp_dre::WarpDRE::new(WarpDREOptionsBuilder::default().build()?);

    let res = aw!(client.get_cached())?;

    assert!(res.ids.len() > 0);
    Ok(())
}

#[test]
#[ignore = "outbound_calls"]
fn get_blacklist() -> anyhow::Result<()> {
    let client = warp_dre::WarpDRE::new(WarpDREOptionsBuilder::default().build()?);

    let res = aw!(client.get_blacklist())?;

    assert!(res.len() > 0);
    Ok(())
}

#[test]
#[ignore = "outbound_calls"]
fn get_errors() -> anyhow::Result<()> {
    let client = warp_dre::WarpDRE::new(WarpDREOptionsBuilder::default().build()?);

    let res = aw!(client.get_errors())?;

    assert!(res.len() > 0);
    Ok(())
}

#[test]
#[ignore = "outbound_calls"]
fn get_contract() -> anyhow::Result<()> {
    let client = warp_dre::WarpDRE::new(WarpDREOptionsBuilder::default().build()?);
    let contract_tx_id = "_z0ch80z_daDUFqC9jHjfOL8nekJcok4ZRkE_UesYsk";
    let res = aw!(client.get_contract(contract_tx_id))?;

    assert!(res.contract_tx_id == contract_tx_id);
    Ok(())
}

#[test]
#[ignore = "outbound_calls"]
fn get_contract_with_query() -> anyhow::Result<()> {
    let client = warp_dre::WarpDRE::new(WarpDREOptionsBuilder::default().build()?);
    let contract_tx_id = "_z0ch80z_daDUFqC9jHjfOL8nekJcok4ZRkE_UesYsk";

    let mut query: HashMap<String, String> = HashMap::new();
    query.insert("query".into(), "$.name".into());

    let res = aw!(client.get_contract_with_query(contract_tx_id, query))?;
    let result = res.result[0].clone();
    let result = result.as_str();
    assert!(result == Some("VouchDAO"));
    Ok(())
}

#[test]
#[ignore = "outbound_calls"]
fn create_interaction() {
    let interactor = interactor::Interactor::new(
        InteractorOptionsBuilder::default()
            .contract_address("yS-CVbsg79p2sSrVAJZyRgE_d90BrxDjpAleRB-ZfXs")
            .build()
            .unwrap(),
    )
    .unwrap();

    let res = aw!(interactor.interact(String::from(
        r#"{"function":"postMessage","content":"Hello world!!!!!!"}"#,
    )));

    match res {
        Ok(r) => {
            println!("{:#?}", r)
        }
        Err(e) => {
            println!("Error: {e}");
            assert_eq!(0, 1)
        }
    }
}
