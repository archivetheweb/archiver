#![feature(fs_try_exists)]
use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::{
    browser::default_executable, protocol::cdp::types::Event, Browser, LaunchOptions,
};
use signal_hook::consts::{SIGINT, SIGTERM};
use std::fs;
use std::process::{self, Command, Stdio};
use std::sync::mpsc::{sync_channel, SyncSender, TryRecvError};
use std::{
    sync::{atomic::AtomicBool, atomic::Ordering, Arc},
    thread::{self, sleep},
    time::Duration,
};

const ARCHIVE_DIR: &str = "archivoor";
const BASE_DIR: &str = "collections";

const BASE_URL: &str = "http://localhost:8080";

fn main() {
    let should_terminate = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate)).unwrap();
    signal_hook::flag::register(SIGINT, Arc::clone(&should_terminate)).unwrap();

    let (tx, rx) = sync_channel(1);

    // first check if we have a collection with wb-manager
    let exists = fs::try_exists(format!("./{}/{}", BASE_DIR, ARCHIVE_DIR)).unwrap();
    if !exists {
        let res = Command::new("wb-manager")
            .args(["init", ARCHIVE_DIR])
            .status()
            .unwrap();

        if !res.success() {
            process::exit(res.code().unwrap());
        }

        fs::create_dir(format!("{}/{}/screenshots", BASE_DIR, ARCHIVE_DIR)).unwrap();
    }

    // then we start the wayback server
    let mut wayback = Command::new("wayback")
        .args(["--record", "--live", "--enable-auto-fetch"])
        .stdout(Stdio::null())
        // .stderr(Stdio::null())
        .spawn()
        .unwrap();

    // TODO ensure the proxy is running
    // we wait for it to start running
    sleep(Duration::from_secs(3));

    let tx1 = tx.clone();

    thread::spawn(move || {
        // TODO crawl logic
        browse("https://archivetheweb.com/", tx1);
    });

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
    wayback.kill().unwrap();
    println!("{}", "Child process killed, goodbye");
}

fn browse(url: &str, tx: SyncSender<String>) {
    let options = LaunchOptions::default_builder()
        .path(Some(default_executable().unwrap()))
        .window_size(Some((1920, 1080)))
        .build()
        .expect("Couldn't find appropriate Chrome binary.");
    let browser = Browser::new(options).unwrap();

    let tab = browser.wait_for_initial_tab().unwrap();

    let url = format!("{}/{}/record/{}", BASE_URL, ARCHIVE_DIR, url);

    tab.navigate_to(&url)
        .unwrap()
        .wait_until_navigated()
        .unwrap();

    let tab2 = tab.clone();

    let sync_event = Arc::new(move |event: &Event| match event {
        Event::PageLifecycleEvent(lifecycle) => {
            println!("{}", lifecycle.params.name);
            if lifecycle.params.name == "networkIdle" {
                let _title = tab2.get_title().unwrap();

                let _png = tab2
                    .capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, false)
                    .unwrap();
                fs::write(
                    format!("{}/{}/screenshots/{}.png", BASE_DIR, ARCHIVE_DIR, "a"),
                    _png,
                )
                .unwrap();

                tx.send("done".to_string()).unwrap();
                return;
            }
        }
        _ => {}
    });

    tab.add_event_listener(sync_event).unwrap();

    sleep(Duration::from_secs(10));
}
