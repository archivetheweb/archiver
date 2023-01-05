#![feature(fs_try_exists)]
use archivoor_v1::crawler::Crawler;
use archivoor_v1::utils::{ARCHIVE_DIR, BASE_DIR, BASE_URL};
use archivoor_v1::warc_writer::Writer;
use log::debug;
use signal_hook::consts::{SIGINT, SIGTERM};
use std::fs;
use std::process::{self, Command};
use std::sync::mpsc::{sync_channel, SyncSender, TryRecvError};
use std::{
    sync::{atomic::AtomicBool, atomic::Ordering, Arc},
    thread::sleep,
    time::Duration,
};

fn setup_dir() -> anyhow::Result<()> {
    // first check if we have a collection with wb-manager
    let exists = fs::try_exists(format!("./{}/{}", BASE_DIR, ARCHIVE_DIR))?;
    if !exists {
        let res = Command::new("wb-manager")
            .args(["init", ARCHIVE_DIR])
            .status()?;

        if !res.success() {
            process::exit(res.code().unwrap());
        }

        fs::create_dir(format!("{}/{}/screenshots", BASE_DIR, ARCHIVE_DIR))?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    debug!("{}", "In debug mode");

    let should_terminate = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate))?;
    signal_hook::flag::register(SIGINT, Arc::clone(&should_terminate))?;

    let (tx, rx) = sync_channel(1);

    setup_dir()?;

    let writer_port = 8080;
    let _writer = Writer::new(writer_port, false)?;

    let tx1: SyncSender<String> = tx.clone();

    let url = format!(
        "{}:{}/{}/record/{}",
        BASE_URL, writer_port, ARCHIVE_DIR, "https://archivetheweb.com"
    );

    let mut crawler = Crawler::new(&url, 1);
    crawler.crawl(tx1).await.unwrap();

    while !should_terminate.load(Ordering::Relaxed) {
        match rx.try_recv() {
            Ok(_res) => {
                // when done, we read the recordings
                break;
            }
            Err(TryRecvError::Empty) => {
                sleep(Duration::from_secs(1));
                continue;
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }

    debug!("{}", "Terminating...");
    // writer.terminate()?;
    debug!("{}", "Child process killed, goodbye");
    Ok(())
}
