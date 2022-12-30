use anyhow::Result;
use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::Tab;
use headless_chrome::{browser::default_executable, Browser, LaunchOptions};
use reqwest::Url;
use std::fs;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::{thread::sleep, time::Duration};

use crate::utils::{ARCHIVE_DIR, BASE_DIR};

const BASE_URL: &str = "http://localhost:8080";

const scroll_js: &str = r#" new Promise((resolve) => {
    var totalHeight = 0;
    var distance = 100;
    var timer = setInterval(() => {
        var scrollHeight = document.body.scrollHeight;
        window.scrollBy(0, distance);
        totalHeight += distance;

        if(totalHeight >= scrollHeight - window.innerHeight){
            clearInterval(timer);
            resolve();
        }
    }, 100);

});"#;

pub struct BrowserController {
    port: u16,
    browser: Browser,
}

impl BrowserController {
    pub fn new(port: u16) -> Result<Self> {
        let options = LaunchOptions::default_builder()
            .path(Some(default_executable().unwrap()))
            .window_size(Some((1920, 1080)))
            .port(Some(port))
            .build()
            .expect("Couldn't find appropriate Chrome binary.");
        let browser = Browser::new(options)?;
        Ok(BrowserController { port, browser })
    }

    pub fn browse(&self, url: &str, tx: SyncSender<String>, screenshot: bool) -> Arc<Tab> {
        let tab = self.browser.wait_for_initial_tab().unwrap();

        let url = format!("{}/{}/record/{}", BASE_URL, ARCHIVE_DIR, url);

        tab.navigate_to(&url)
            .unwrap()
            .wait_until_navigated()
            .unwrap();

        // to do, have a better wait function
        tab.wait_for_element("a").unwrap();

        sleep(Duration::from_secs(1));

        if screenshot {
            let _png = tab
                .capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, false)
                .unwrap();
            fs::write(
                format!("{}/{}/screenshots/{}.png", BASE_DIR, ARCHIVE_DIR, "a"),
                _png,
            )
            .unwrap();
        }

        println!("scrolling....");

        let r = tab.evaluate(scroll_js, true).unwrap();

        println!("{:?}", r);

        sleep(Duration::from_secs(5));
        tx.send("done".to_string()).unwrap();

        tab
    }

    pub fn get_links(&self, tab: &Arc<Tab>) -> Vec<String> {
        let rs = tab.find_elements("a").unwrap();

        let links = rs
            .iter()
            .map(|x| x.get_attributes().unwrap().unwrap())
            .filter_map(|x| {
                let mut peekable = x.into_iter().peekable();

                for elem in peekable.next() {
                    if elem == "href".to_string() {
                        return peekable.next();
                    }
                }

                None
            })
            .filter_map(normalize_url(BASE_URL.to_string()))
            .collect::<Vec<String>>();

        links
    }
}

fn normalize_url(base_url: String) -> Box<dyn Fn(String) -> Option<String>> {
    return Box::new(move |url| {
        let new_url = Url::parse(url.as_str());
        match new_url {
            Ok(new_url) => Some(new_url.to_string()),
            Err(_e) => {
                if url.starts_with('/') {
                    Some(format!("{}{}", base_url, url))
                } else {
                    None
                }
            }
        }
    });
}
