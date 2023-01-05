use anyhow::{Context, Result};
use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::Tab;
use headless_chrome::{browser::default_executable, Browser, LaunchOptions};
use std::fs;
use std::sync::Arc;
use std::{thread::sleep, time::Duration};
use sysinfo::{Pid, PidExt, ProcessExt, System, SystemExt};

use crate::utils::{ARCHIVE_DIR, BASE_DIR};

const SCROLL_JS: &str = r#" new Promise((resolve) => {
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
    browser: Browser,
}

impl BrowserController {
    pub fn new() -> Result<Self> {
        let options = LaunchOptions::default_builder()
            .path(Some(default_executable().unwrap()))
            .window_size(Some((1920, 1080)))
            .build()
            .expect("Couldn't find appropriate Chrome binary.");
        let browser = Browser::new(options).context("browser error")?;

        Ok(BrowserController { browser })
    }

    pub fn browse(&self, url: &str, screenshot: bool) -> Arc<Tab> {
        let tab = self.browser.wait_for_initial_tab().unwrap();

        let url = format!("{}", url);

        tab.navigate_to(&url)
            .unwrap()
            .wait_until_navigated()
            .unwrap();

        // to do, have a better wait function
        tab.wait_for_element("a").unwrap();

        debug!("sleeping for 1 second");

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

        debug!("scrolling....");

        let _r = tab.evaluate(SCROLL_JS, true).unwrap();

        debug!("sleeping for 3 seconds");

        sleep(Duration::from_secs(3));

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
            .collect::<Vec<String>>();

        links
    }

    pub fn kill(&self) -> bool {
        let pid = self.browser.get_process_id().unwrap();
        let s = System::new();
        if let Some(process) = s.process(Pid::from_u32(pid)) {
            debug!("killing process with id {}", pid);
            process.kill();
            return true;
        }
        false
    }
}

impl Drop for BrowserController {
    fn drop(&mut self) {
        debug!("killing browser process...");
        self.kill();
    }
}
