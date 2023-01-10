use archivoor_v1::runner::{LaunchOptionsBuilder, Runner};
use log::debug;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    debug!("{}", "In debug mode");

    let options = LaunchOptionsBuilder::default().with_upload(true).build()?;

    debug!("Launching app with options: \n {:#?}", options);

    let r = Runner::new(options).await?;

    r.run("https://archivetheweb.com").await?;

    Ok(())
}
