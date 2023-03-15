use std::{path::PathBuf, time::Duration};

use archiver::runner::{Runner, RunnerOptions};
use headless_chrome::{browser::default_executable, Browser, LaunchOptions};
macro_rules! aw {
    ($e:expr) => {
        tokio_test::block_on($e)
    };
}

/*
RUST_LOG=debug cargo test --package archiver --test crawl --   crawl_website --exact --ignored
 */
#[test]
#[ignore = "crawl"]
fn crawl_website() -> anyhow::Result<()> {
    env_logger::init();
    let options = RunnerOptions::default_builder()
        .writer_dir(Some(PathBuf::from(".")))
        .concurrent_tabs(10)
        .url_retries(2)
        .with_upload(false)
        .writer_port(None)
        .writer_debug(false)
        .archive_name(None)
        .crawl_depth(0)
        .timeout(45u64)
        .min_wait_after_navigation(5u64)
        .max_wait_after_navigation(7u64)
        // .domain_only(req.options.domain_only)
        .build()?;
    let runner = aw!(Runner::new(options))?;
    let res = aw!(runner.run_archiving("https://www.adobe.com/"));
    println!("{res:#?}");
    Ok(())
}

#[test]
#[ignore = "crawl"]
fn headless_chrome() -> anyhow::Result<()> {
    env_logger::init();
    let options = LaunchOptions::default_builder()
        .path(Some(default_executable().unwrap()))
        .window_size(Some((1920, 1080)))
        .idle_browser_timeout(Duration::from_secs(45))
        .sandbox(true)
        .build()
        .expect("Couldn't find appropriate Chrome binary.");
    let browser = Browser::new(options)?;
    let ctx = browser.new_context()?;
    let tab = ctx.new_tab()?;
    let nv = tab.navigate_to("")?;
    nv.wait_until_navigated()?;
    let elems = nv.find_elements("a")?;
    println!("{elems:?}");

    Ok(())
}
