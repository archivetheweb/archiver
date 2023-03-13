use anyhow::{Context, Result};
use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::Tab;
use headless_chrome::{browser::default_executable, Browser, LaunchOptions};
use rand::Rng;
use std::fs;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
use sysinfo::{Pid, PidExt, ProcessExt, System, SystemExt};

use crate::utils::{extract_collection_name, get_tmp_screenshot_dir};

fn get_scroll_script(timeout_ms: i128) -> String {
    format!(
        r#" new Promise((resolve) => {{
            var totalHeight = 0;
            var distance = 100;
            var timer = setInterval(() => {{
                var scrollHeight = document.body.scrollHeight;
                window.scrollBy(0, distance);
                totalHeight += distance;
        
                if(totalHeight >= scrollHeight - window.innerHeight){{
                    clearInterval(timer);
                    resolve("ok");
                }}
            }}, {});
        }});"#,
        timeout_ms
    )
}

pub struct BrowserController {
    browser: Browser,
    min_wait_secs: u64,
    max_wait_secs: u64,
}

impl BrowserController {
    pub fn new(idle_browser_timeout: u64, min_wait_secs: u64, max_wait_secs: u64) -> Result<Self> {
        let is_docker = std::env::var("IN_DOCKER").is_ok();
        let options = LaunchOptions::default_builder()
            .path(Some(default_executable().unwrap()))
            .window_size(Some((1920, 1080)))
            .idle_browser_timeout(Duration::from_secs(idle_browser_timeout))
            // warning only do this if in docker env
            .sandbox(!is_docker)
            .build()
            .expect("Couldn't find appropriate Chrome binary.");
        let browser = Browser::new(options).context("browser launching error")?;

        Ok(BrowserController {
            browser,
            min_wait_secs,
            max_wait_secs,
        })
    }

    pub fn browse(&self, url: &str, screenshot: bool) -> anyhow::Result<Arc<Tab>> {
        // we create a new incognito window (no context)
        let ctx = self
            .browser
            .new_context()
            .context("could not create incognito context")?;
        let tab = ctx.new_tab().context("could not create new tab")?;

        let nv = match tab.navigate_to(&url) {
            Ok(t) => t,
            Err(e) => {
                error!("could not navigate to {} with error {}", url, e);
                tab.navigate_to(&url)?
            }
        };
        if let Err(e) = nv.wait_until_navigated() {
            // we wait one more timeout
            warn!("error waiting for navigation, retrying {}", e);
            nv.wait_until_navigated()?;
        }

        let rndm = {
            let mut rng = rand::thread_rng();
            rng.gen_range(self.min_wait_secs..self.max_wait_secs)
        };
        debug!("successfully navigated, sleeping for {} seconds", rndm);
        sleep(Duration::from_secs(rndm));

        if screenshot {
            let collection_name = extract_collection_name(&url);
            debug!("taking screenshot of {}", &url);

            let _png = tab
                .capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, false)
                .context(format!("screenshot for {} could not be captured", &url))?;
            let filename = get_tmp_screenshot_dir(&collection_name);
            debug!("saving temporary screenshot to {}", filename);

            fs::write(filename, _png).context(format!("could not save screenshot for {}", &url))?;
        }

        debug!("scrolling....");
        match tab.evaluate(&get_scroll_script(60), true) {
            Ok(_) => {}
            Err(_) => {
                // we retry
                warn!("Scrolling for url {} is retrying", url);
                tab.evaluate(&get_scroll_script(30), true)?;
            }
        };
        debug!("successfully scrolled, sleeping for {} seconds", rndm);
        sleep(Duration::from_secs(rndm));

        Ok(tab)
    }

    pub fn get_links(&self, tab: &Arc<Tab>) -> Vec<String> {
        let rs = match tab.find_elements("a") {
            Ok(elems) => elems,
            Err(e) => {
                error!("could not get link for {} with error {}", tab.get_url(), e);
                vec![]
            }
        };

        let links = rs
            .iter()
            .map(|x| {
                x.get_attributes()
                    .context(format!(
                        "could not get attributes for url {}",
                        tab.get_url()
                    ))
                    .unwrap()
                    .unwrap()
            })
            .filter_map(|x| {
                // TODO allow to filter specific links (filter mailto: etc)
                for i in 0..x.len() {
                    if x[i] == "href" {
                        return Some(x[i + 1].clone());
                    }
                }
                None
            })
            .collect::<Vec<String>>();
        links
    }

    pub fn kill(&self) -> bool {
        let pid = self
            .browser
            .get_process_id()
            .context("could not get process id for browser")
            .unwrap();
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
