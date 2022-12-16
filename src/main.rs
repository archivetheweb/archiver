#![feature(fs_try_exists)]
use headless_chrome::Browser;
use signal_hook::consts::SIGINT;
use signal_hook::consts::SIGTERM;
use std::fs;
use std::process::{self, Command, Stdio};
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};
use std::{thread::sleep, time::Duration};

const ARCHIVE_DIR: &str = "archivoor";
const BASE_DIR: &str = "collections";

fn main() {
    let should_terminate = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate)).unwrap();
    signal_hook::flag::register(SIGINT, Arc::clone(&should_terminate)).unwrap();

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
        .args(["--record", "--live", "-a"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    sleep(Duration::from_secs(2));

    browse("https://wikipedia.org");

    while !should_terminate.load(Ordering::Relaxed) {}

    println!("{}", "Terminating...");
    wayback.kill().unwrap();
    println!("{}", "wayback killed");
}

fn browse(url: &str) {
    let browser = Browser::default().unwrap();

    let tab = browser.wait_for_initial_tab().unwrap();

    let url = format!("http://localhost:8080/{}/record/{}", ARCHIVE_DIR, url);

    println!("{}", url);
    tab.navigate_to(&url)
        .unwrap()
        .wait_until_navigated()
        .unwrap();
}
