#![feature(fs_try_exists)]
use headless_chrome::{
    browser::default_executable, protocol::cdp::types::Event, Browser, LaunchOptions,
};
use signal_hook::consts::SIGINT;
use signal_hook::consts::SIGTERM;
use std::fs;
use std::process::{self, Command, Stdio};
use std::sync::mpsc::sync_channel;
use std::sync::mpsc::SyncSender;
use std::sync::mpsc::TryRecvError;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};
use std::thread;
use std::{thread::sleep, time::Duration};

const ARCHIVE_DIR: &str = "archivoor";
const BASE_DIR: &str = "collections";

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
            println!("{}", "Problem initializing collection directory");
            process::exit(res.code().unwrap());
        }
    }

    // then we start the wayback server
    let mut wayback = Command::new("wayback")
        .args(["--record", "--live", "--enable-auto-fetch"])
        .stdout(Stdio::null())
        // .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    sleep(Duration::from_secs(2));

    let tx1 = tx.clone();

    thread::spawn(move || {
        // TODO crawl logic
        // browse("https://wikipedia.org");
        // browse("https://en.wikipedia.org");
        browse("https://archivetheweb.com/", tx1);
        println!("{}", "navigation ended");

        tx.send("done".to_string()).unwrap();
    });

    while !should_terminate.load(Ordering::Relaxed) {
        match rx.try_recv() {
            Ok(res) => {
                println!("{}", res);

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
    println!("{}", "wayback killed");
}

fn browse(url: &str, tx: SyncSender<String>) {
    // let browser = Browser::default().unwrap();
    let options = LaunchOptions::default_builder()
        .path(Some(default_executable().unwrap()))
        .window_size(Some((1920, 1080)))
        .build()
        .expect("Couldn't find appropriate Chrome binary.");
    let browser = Browser::new(options).unwrap();

    let tab = browser.wait_for_initial_tab().unwrap();

    let url = format!("http://localhost:8080/{}/record/{}", ARCHIVE_DIR, url);

    tab.navigate_to(&url)
        .unwrap()
        .wait_until_navigated()
        .unwrap();

    let title = tab.get_title().unwrap();

    let sync_event = Arc::new(move |event: &Event| match event {
        Event::PageLifecycleEvent(lifecycle) => {
            println!("{}", lifecycle.params.name);
            if lifecycle.params.name == "networkIdle" {
                println!("{}", "networkIdle");
                tx.send("done".to_string()).unwrap();
                return;
            }
        }
        _ => {}
    });

    tab.add_event_listener(sync_event).unwrap();

    println!(" title is {}", title);

    let element = tab.wait_for_element("#replay_iframe").unwrap();

    let _js_call = element.call_js_fn(
        "function () {
        let h = document.getElementById('replay_iframe');
        h.contentWindow.scrollTo({ left: 0, top: 1000000, behavior: 'smooth' });
        return 41;
    }",
        vec![],
        false,
    );

    sleep(Duration::from_secs(10));
}
