use futures::StreamExt;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::SyncSender,
        Arc,
    },
    time::Duration,
};
use tokio::{sync::mpsc, task, time::sleep};

use crate::{
    browser_controller::BrowserController,
    utils::{normalize_url, BASE_URL},
};

pub struct Crawler {
    visited: HashSet<String>,
    failed: HashMap<String, i32>,
    depth: i32,
    url: String,
    concurrent_browsers: usize,
    url_retries: i32,
}

// the crawler sets up the necessary data types to store the crawl information.

// will say it is done, when done we move on to the next process (compression, arweave deployment etc)

impl Crawler {
    pub fn new(url: &str, depth: i32, concurrent_browsers: usize, url_retries: i32) -> Crawler {
        Crawler {
            visited: HashSet::new(),
            failed: HashMap::new(),
            depth,
            url: url.to_string(),
            concurrent_browsers,
            url_retries,
        }
    }

    pub async fn crawl(
        &mut self,
        tx: SyncSender<String>,
        should_terminate: Arc<AtomicBool>,
    ) -> anyhow::Result<()> {
        // we setup a channel for new url
        // this channel will send an (String, Vec<String>,i32) tuple
        // first element being the url visited, next element being all the new urls and last being the depth of the visited_url
        let (scraped_urls_tx, mut scraped_urls_rx) =
            mpsc::channel::<(String, Vec<String>, i32)>(100);
        let (visit_url_tx, visit_url_rx) = mpsc::channel::<(String, i32)>(100);
        let (failed_url_tx, mut failed_url_rx) = mpsc::channel::<(String, i32)>(100);

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
                    "Adding {} to a visited url at depth {}",
                    &visited_url, depth
                );
                self.visited.insert(visited_url);
                let new_urls: HashSet<String> = HashSet::from_iter(new_scraped_urls.into_iter());
                for new_url in new_urls.iter() {
                    if !self.visited.contains(new_url) && depth < self.depth {
                        debug!("Adding {} to the queue", &new_url);
                        let _ = visit_url_tx.send((new_url.to_string(), depth + 1)).await;
                    }
                }
            } else {
                // we check if channel is empty
                match res.err().unwrap() {
                    mpsc::error::TryRecvError::Empty => {
                        // debug!("empty")
                    }
                    mpsc::error::TryRecvError::Disconnected => debug!("disconnected"),
                }
            }

            // we retry
            if self.url_retries > 0 {
                match failed_url_rx.try_recv() {
                    Ok((url, depth)) => {
                        match self.failed.get_mut(&url) {
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
                                self.failed.insert(url.clone(), 0);
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

            if scraped_urls_tx.capacity() == 100
                && visit_url_tx.capacity() == 100
                && active_browsers.load(Ordering::SeqCst) == 0
            {
                debug!("crawl of {} completed successfully", self.url);

                break;
            }

            sleep(Duration::from_millis(10)).await;
        }

        tx.send("done".to_string()).unwrap();

        Ok(())
    }

    fn processor(
        &self,
        scraped_urls_tx: mpsc::Sender<(String, Vec<String>, i32)>,
        visit_url_rx: mpsc::Receiver<(String, i32)>,
        failed_url_tx: mpsc::Sender<(String, i32)>,
        active_browsers: Arc<AtomicUsize>,
    ) {
        debug!("processing....");
        let concurrency = self.concurrent_browsers;
        tokio::spawn(async move {
            tokio_stream::wrappers::ReceiverStream::new(visit_url_rx)
                .for_each_concurrent(concurrency, |queued_url| {
                    let (url, depth) = queued_url.clone();
                    let ab = active_browsers.clone();
                    let tx = scraped_urls_tx.clone();
                    let failed_url_tx = failed_url_tx.clone();
                    debug!("browsing {} at depth {}", url, depth);
                    let u = url.clone();
                    async move {
                        ab.fetch_add(1, Ordering::SeqCst);

                        let links: Result<_, anyhow::Error> = task::spawn_blocking(move || {
                            let browser = match BrowserController::new() {
                                Ok(b) => b,
                                Err(_) => return Ok((vec![], true)),
                            };

                            let tab = match browser.browse(&u, false) {
                                Ok(tab) => tab,
                                Err(_) => {
                                    warn!("error browsing for {}", u);
                                    // we return an empty list of links, and flag as errored out
                                    return Ok((vec![], true));
                                }
                            };

                            Ok((
                                browser
                                    .get_links(&tab)
                                    .iter()
                                    .filter_map(normalize_url(format!("{}:{}", BASE_URL, 8080)))
                                    .collect::<Vec<String>>(),
                                false,
                            ))
                        })
                        .await
                        .unwrap();

                        let links = links.unwrap();
                        // if there was an error
                        // we send it to the failed url channel
                        if links.1 {
                            failed_url_tx.send((url, depth)).await.unwrap();
                        } else {
                            tx.send((url, links.0, depth)).await.unwrap();
                        }
                        ab.fetch_sub(1, Ordering::SeqCst);
                    }
                })
                .await;

            return;
        });
    }
}
