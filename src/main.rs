#![feature(fs_try_exists)]
use archivoor_v1::browser_controller::BrowserController;
use archivoor_v1::uploader::Uploader;
use archivoor_v1::utils::{ARCHIVE_DIR, BASE_DIR};
use archivoor_v1::warc_writer::Writer;
use signal_hook::consts::{SIGINT, SIGTERM};
use std::collections::HashSet;
use std::fs;
use std::process::{self, Command};
use std::sync::mpsc::{sync_channel, SyncSender, TryRecvError};
use std::{
    sync::{atomic::AtomicBool, atomic::Ordering, Arc},
    thread::{self, sleep},
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

fn main() -> anyhow::Result<()> {
    let should_terminate = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate))?;
    signal_hook::flag::register(SIGINT, Arc::clone(&should_terminate))?;

    let (tx, rx) = sync_channel(1);

    setup_dir()?;

    let visited: Arc<HashSet<String>> = Arc::new(HashSet::new());

    // let writer = Writer::new(8080, false)?;

    let tx1: SyncSender<String> = tx.clone();

    let browser = BrowserController::new(8112)?;

    thread::spawn(move || {
        // TODO crawl logic
        // let tab = browser.browse("https://bbc.com/", tx1, true);
        // browser.get_links(&tab);
    });

    let up = Uploader::new();
    up.fetch_latest_warc()?;

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

    println!("{}", "Terminating...");
    // writer.terminate()?;
    println!("{}", "Child process killed, goodbye");
    Ok(())
}
