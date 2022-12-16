#![feature(fs_try_exists)]
use headless_chrome::protocol::cdp::types::Event;
use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::Browser;
use signal_hook::consts::SIGINT;
use signal_hook::consts::SIGTERM;
use std::fs;
use std::process::{self, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};
use std::thread;
use std::{thread::sleep, time::Duration};

const ARCHIVE_DIR: &str = "archivoor";
const BASE_DIR: &str = "collections";

fn main() {
    let should_terminate = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate)).unwrap();
    signal_hook::flag::register(SIGINT, Arc::clone(&should_terminate)).unwrap();

    let (tx, rx) = channel();

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
        // .args(["--record", "--live", "-a"])
        // .args(["--record", "--live"])
        .stdout(Stdio::null())
        // .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    sleep(Duration::from_secs(2));

    thread::spawn(move || {
        // TODO crawl logic
        // browse("https://wikipedia.org");
        // browse("https://en.wikipedia.org");
        browse("https://archivetheweb.com/");
        sleep(Duration::from_secs(2));

        println!("{}", "navigation ended");

        tx.send("done").unwrap();
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

fn browse(url: &str) {
    let browser = Browser::default().unwrap();
    /*     let browser = Browser::new(
        LaunchOptionsBuilder::default()
            .headless(true)
            .build()
            .unwrap(),
    )
    .unwrap(); */

    let tab = browser.wait_for_initial_tab().unwrap();

    let url = format!("http://localhost:8080/{}/record/{}", ARCHIVE_DIR, url);

    tab.navigate_to(&url)
        .unwrap()
        .wait_until_navigated()
        .unwrap();

    tab.bring_to_front().unwrap();

    let title = tab.get_title().unwrap();

    println!(" title is {}", title);

    // sleep(Duration::from_secs(5))

    let sync_event = Arc::new(move |event: &Event| match event {
        Event::PageLifecycleEvent(lifecycle) => {
            if lifecycle.params.name == "DOMContentLoaded" {
                println!("{}", "loaded");
            }
        }
        _ => {}
    });

    tab.add_event_listener(sync_event).unwrap();

    // let element = tab.wait_for_element("wb_iframe_div").unwrap();

    let func = "
    (function () { 
        let h = document.getElementById('replay_iframe');
        h.contentWindow.scrollTo({ left: 0, top: document.body.scrollHeight, behavior: 'smooth' });
        return 41;
    })();";

    let rem = tab.evaluate(func, true).unwrap();

    println!("{}", rem.description.unwrap());

    sleep(Duration::from_secs(2));

    let _png = tab
        .capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, false)
        .unwrap();
    fs::write("a.png", _png).unwrap();
}
