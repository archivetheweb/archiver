use futures::StreamExt;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{sync::mpsc, time::sleep};

use crate::{
    browser_controller::BrowserController,
    types::CrawlResult,
    utils::{extract_url, normalize_url_map},
};

pub struct Crawler {
    visited: HashSet<String>,
    failed: HashMap<String, i32>,
    depth: i32,
    base_url: String,
    url: String,
    concurrent_browsers: i32,
    url_retries: i32,
    main_title: Arc<tokio::sync::Mutex<String>>,
}

impl Crawler {
    pub fn new(
        base_url: &str,
        url: &str,
        depth: i32,
        concurrent_browsers: i32,
        url_retries: i32,
    ) -> Crawler {
        Crawler {
            visited: HashSet::new(),
            failed: HashMap::new(),
            base_url: base_url.into(),
            depth,
            url: url.into(),
            concurrent_browsers,
            url_retries,
            main_title: Arc::new(tokio::sync::Mutex::new(String::from(""))),
        }
    }

    pub async fn crawl(
        &mut self,
        should_terminate: Arc<AtomicBool>,
    ) -> anyhow::Result<CrawlResult> {
        // we setup a channel for new url
        // this channel will send an (String, Vec<String>,i32) tuple
        // first element being the url visited, next element being all the new urls and last being the depth of the visited_url
        let (scraped_urls_tx, mut scraped_urls_rx) =
            mpsc::channel::<(String, Vec<String>, i32)>(self.concurrent_browsers as usize + 10);
        // TODO the 1000 is only temporary
        let (visit_url_tx, visit_url_rx) = mpsc::channel::<(String, i32)>(1000);
        let (failed_url_tx, mut failed_url_rx) = mpsc::channel::<(String, i32)>(1000);

        let active_browsers = Arc::new(AtomicUsize::new(0));

        self.processor(
            scraped_urls_tx.clone(),
            visit_url_rx,
            failed_url_tx,
            active_browsers.clone(),
        );

        // we send the first url to crawl
        visit_url_tx.send((self.url.clone(), 0)).await.unwrap();

        while !should_terminate.load(Ordering::Relaxed) {
            // we take in new urls
            let res = scraped_urls_rx.try_recv();

            if res.is_ok() {
                let (visited_url, new_scraped_urls, depth) = res.unwrap();
                debug!(
                    "Adding {} as a visited url at depth {}",
                    &visited_url, depth
                );
                self.visited.insert(visited_url.to_string());
                let new_urls: HashSet<String> = HashSet::from_iter(new_scraped_urls);
                for new_url in new_urls.iter() {
                    if !self.visited.contains(new_url) && depth < self.depth {
                        debug!("Adding {} to the queue", &new_url);
                        let _ = visit_url_tx.send((new_url.to_string(), depth + 1)).await;
                    }
                }
            } else {
                // we check if channel is empty
                match res.err().unwrap() {
                    mpsc::error::TryRecvError::Empty => {}
                    mpsc::error::TryRecvError::Disconnected => debug!("disconnected"),
                }
            }

            if self.url_retries > 0 {
                match failed_url_rx.try_recv() {
                    Ok((url, depth)) => {
                        match self.failed.get_mut(&url.to_string()) {
                            Some(count) if count <= &mut self.url_retries => {
                                warn!(
                                    "Retrying url {} at d={}, retried {} so far",
                                    url, depth, count
                                );
                                // we resend the url to be fetched
                                visit_url_tx.send((url, depth)).await.unwrap();
                                *count = *count + 1;
                            }
                            None => {
                                warn!("Retrying url {} at d={}, retried {} so far", url, depth, 0);
                                self.failed.insert(url.to_string(), 0);
                                // this could be blocking if not in it's own thread or not enough buffer
                                visit_url_tx.send((url, depth)).await.unwrap();
                            }
                            _ => {
                                error!("url {} could not be retrieved", url);
                            }
                        }
                    }
                    Err(_) => {}
                }
            }

            if scraped_urls_tx.capacity() == scraped_urls_tx.max_capacity()
                && visit_url_tx.capacity() == visit_url_tx.max_capacity()
                && active_browsers.load(Ordering::SeqCst) == 0
            {
                break;
            }

            sleep(Duration::from_millis(10)).await;
        }
        let failed = self
            .failed
            .iter()
            .filter_map(|x| {
                if x.1 >= &self.url_retries {
                    return Some(x.0.to_owned());
                }
                None
            })
            .collect::<HashSet<String>>();

        debug!(
            "Total of {} sites crawled, {} failed",
            self.visited.len(),
            failed.len()
        );

        if failed.len() > 0 {
            debug!("Failed urls: {:#?}", failed);
        }

        let url = extract_url(&self.url);
        info!("crawl of {} completed successfully", extract_url(&self.url));

        return Ok(CrawlResult {
            failed,
            visited: self.visited.clone(),
            url,
            main_title: self.main_title.lock().await.to_string(),
        });
    }

    fn processor(
        &self,
        scraped_urls_tx: mpsc::Sender<(String, Vec<String>, i32)>,
        visit_url_rx: mpsc::Receiver<(String, i32)>,
        failed_url_tx: mpsc::Sender<(String, i32)>,
        active_browsers: Arc<AtomicUsize>,
    ) {
        let concurrency = self.concurrent_browsers;
        let base_url = self.base_url.clone();
        let start_url = self.url.clone();
        let m = self.main_title.clone();
        tokio::spawn(async move {
            tokio_stream::wrappers::ReceiverStream::new(visit_url_rx)
                .for_each_concurrent(concurrency as usize, |queued_url| {
                    let (url, depth) = queued_url.clone();
                    debug!("browsing {} at depth {}", url, depth);

                    let ab = active_browsers.clone();
                    let tx = scraped_urls_tx.clone();
                    let failed_url_tx = failed_url_tx.clone();
                    let u = url.clone();
                    let base_url = base_url.clone();
                    let is_first_url = start_url == u;
                    let title_mutex = m.clone();

                    async move {
                        ab.fetch_add(1, Ordering::SeqCst);

                        let links = async move {
                            // headless chrome can't handle pdfs, so we make a direct request for it
                            if u.as_str().ends_with(".pdf") {
                                match reqwest::blocking::get(u.as_str()) {
                                    Ok(res) => {
                                        // make sure we read the text
                                        debug!("fetching pdf at {}", u.as_str());
                                        let _r = res.text();
                                        return (vec![], false);
                                    }
                                    Err(e) => {
                                        warn!("error downloading pdf err: {}", e);
                                        return (vec![], true);
                                    }
                                }
                            }

                            let browser = match BrowserController::new().await {
                                Ok(b) => b,
                                Err(_) => return (vec![], true),
                            };
                            let tab = match browser.browse(u.as_str(), is_first_url).await {
                                Ok(tab) => tab,
                                Err(e) => {
                                    warn!("error browsing for {} with err {}", u, e);
                                    // we return an empty list of links, and flag as errored out
                                    return (vec![], true);
                                }
                            };

                            if is_first_url {
                                let title = match tab.get_title() {
                                    Ok(t) => t,
                                    Err(e) => {
                                        warn!("could not get title {:?}", e);
                                        "".into()
                                    }
                                };
                                let mut main_title = title_mutex.blocking_lock();
                                *main_title = title;
                            }

                            (
                                browser
                                    .get_links(&tab)
                                    .await
                                    .iter()
                                    .filter_map(normalize_url_map(base_url.into()))
                                    .collect::<Vec<String>>(),
                                false,
                            )
                        }
                        .await;

                        // the boolean in the second element of the tuple
                        // tells us whether there was an error or not
                        // if so, we send the url to the failed url channel
                        if links.1 {
                            match failed_url_tx.send((url, depth)).await {
                                Ok(_) => {}
                                Err(e) => {
                                    error!("could not send to failed_url_tx {}", e)
                                }
                            };
                        } else {
                            match tx.send((url, links.0, depth)).await {
                                Ok(_) => {}
                                Err(e) => {
                                    error!("could not send to tx {}", e)
                                }
                            };
                        }
                        ab.fetch_sub(1, Ordering::SeqCst);
                    }
                })
                .await;

            return;
        });
    }
    pub fn url(&self) -> String {
        self.url.to_string()
    }
}
