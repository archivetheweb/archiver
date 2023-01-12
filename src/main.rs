use archivoor_v1::runner::{LaunchOptions, Runner};
use log::debug;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    debug!("{}", "In debug mode");

    let options = LaunchOptions::default_builder()
        .with_upload(false)
        .writer_dir(Some(".".into()))
        .writer_port(None)
        .writer_debug(false)
        .archive_name(None)
        .crawl_depth(1)
        .concurrent_browsers(10)
        .build()?;

    debug!("Launching app with options: \n {:#?}", options);

    let r = Runner::new(options).await?;

    r.run("https://archivetheweb.com").await?;
    // r.run("https://bbc.com").await?;

    Ok(())
}
