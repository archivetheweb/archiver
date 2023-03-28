use atw::state::CrawlType;
use futures::StreamExt;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{sync::mpsc, task, time::sleep};

use crate::{
    browser_controller::BrowserController,
    types::{BrowsingResult, CrawlRequest, CrawlResult, PageCrawlResult, UrlInfo},
    utils::{extract_url, get_domain, normalize_url_map},
};

pub struct Crawler {
    visited: HashSet<String>,
    visiting: HashSet<String>,
    failed: HashMap<String, i32>,
    depth: i32,
    crawl_type: CrawlType,
    base_url: String,
    url: String,
    original_url: String,
    concurrent_tabs: i32,
    url_retries: i32,
    main_title: Arc<tokio::sync::Mutex<String>>,
    timeout: u64,
    min_wait_secs: u64,
    max_wait_secs: u64,
}

impl Crawler {
    pub fn new(
        base_url: &str,
        full_url: &str,
        original_url: &str,
        depth: i32,
        crawl_type: CrawlType,
        concurrent_tabs: i32,
        url_retries: i32,
        timeout: u64,
        min_wait_secs: u64,
        max_wait_secs: u64,
    ) -> Crawler {
        Crawler {
            visited: HashSet::new(),
            visiting: HashSet::new(),
            failed: HashMap::new(),
            base_url: base_url.into(),
            crawl_type,
            depth,
            url: full_url.into(),
            original_url: original_url.into(),
            concurrent_tabs,
            url_retries,
            main_title: Arc::new(tokio::sync::Mutex::new(String::from(""))),
            timeout,
            min_wait_secs,
            max_wait_secs,
        }
    }

    pub async fn crawl(
        &mut self,
        should_terminate: Arc<AtomicBool>,
    ) -> anyhow::Result<CrawlResult> {
        let (scraped_urls_tx, mut scraped_urls_rx) =
            mpsc::channel::<PageCrawlResult>(self.concurrent_tabs as usize + 10);

        let (visit_url_tx, visit_url_rx) = mpsc::channel::<CrawlRequest>(1000);
        let (failed_url_tx, mut failed_url_rx) = mpsc::channel::<CrawlRequest>(1000);

        let active_tabs = Arc::new(AtomicUsize::new(0));

        self.processor(
            scraped_urls_tx.clone(),
            visit_url_rx,
            failed_url_tx,
            active_tabs.clone(),
        );

        visit_url_tx
            .send(CrawlRequest::new(self.url.clone(), 0))
            .await
            .unwrap();

        let domain = get_domain(&self.original_url).unwrap();

        while !should_terminate.load(Ordering::Relaxed) {
            let res = scraped_urls_rx.try_recv();

            if res.is_ok() {
                let crawl_result = res.unwrap();
                let visited_url = crawl_result.visited_url();
                let depth = crawl_result.depth();
                debug!(
                    "adding {} as a visited url at depth {}",
                    &visited_url,
                    crawl_result.depth()
                );
                self.visited.insert(visited_url.clone());
                self.visiting.remove(&visited_url);
                let new_urls: HashSet<UrlInfo> = HashSet::from_iter(crawl_result.links());
                for new_url in new_urls.iter() {
                    if !self.visited.contains(&new_url.url)
                        && !self.visiting.contains(&new_url.url)
                        && depth < self.depth
                    {
                        match self.crawl_type {
                            CrawlType::DomainOnly => {
                                if new_url.domain != domain {
                                    continue;
                                }
                            }
                            CrawlType::DomainWithPageLinks => {
                                if get_domain(&extract_url(&visited_url)).unwrap() != domain {
                                    continue;
                                }
                            }
                            CrawlType::DomainAndLinks => {}
                        }

                        debug!("adding {} to the queue", &new_url.url);
                        self.visiting.insert(new_url.url.clone());
                        match visit_url_tx
                            .send(CrawlRequest::new(new_url.url.to_string(), depth + 1))
                            .await
                        {
                            Ok(_) => {}
                            Err(e) => {
                                error!(
                                    "could not send new_url:{} to visit_url_tx {}",
                                    new_url.url, e
                                )
                            }
                        };
                    }
                }
            }

            if self.url_retries > 0 {
                match failed_url_rx.try_recv() {
                    Ok(crawl_request) => {
                        let url = crawl_request.url();
                        let depth = crawl_request.depth();
                        self.visiting.remove(&url);
                        match self.failed.get_mut(&url.to_string()) {
                            Some(count) if count <= &mut self.url_retries => {
                                warn!(
                                    "retrying url {} at d={}, retried {} so far",
                                    url, depth, count
                                );
                                match visit_url_tx
                                    .send(CrawlRequest::new(url.clone(), depth))
                                    .await
                                {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!(
                                            "could not send url {} to visit_url_tx for retry {} {}",
                                            url, count, e
                                        )
                                    }
                                };
                                *count = *count + 1;
                            }
                            None => {
                                warn!("first retry of url {} at d={}", url, depth);
                                self.failed.insert(url.to_string(), 0);
                                match visit_url_tx
                                    .send(CrawlRequest::new(url.clone(), depth))
                                    .await
                                {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("could not send url {} to visit_url_tx for first try {}", url, e)
                                    }
                                };
                            }
                            _ => {
                                error!("url {} could not be retrieved", url);
                            }
                        }
                    }
                    Err(_) => {}
                }
            }

            // if all of our channels are at capacity and we have no active tabs, we are done
            if scraped_urls_tx.capacity() == scraped_urls_tx.max_capacity()
                && visit_url_tx.capacity() == visit_url_tx.max_capacity()
                && active_tabs.load(Ordering::SeqCst) == 0
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

        Ok(CrawlResult::new(
            url,
            self.main_title.lock().await.to_string(),
            self.visited.clone(),
            failed,
        ))
    }

    fn processor(
        &self,
        scraped_urls_tx: mpsc::Sender<PageCrawlResult>,
        visit_url_rx: mpsc::Receiver<CrawlRequest>,
        failed_url_tx: mpsc::Sender<CrawlRequest>,
        active_tabs: Arc<AtomicUsize>,
    ) {
        let concurrent_tabs = self.concurrent_tabs;
        let base_url = self.base_url.clone();
        let original_url = self.url.clone();
        let title = self.main_title.clone();
        let min_wait = self.min_wait_secs;
        let max_wait = self.max_wait_secs;
        let timeout = self.timeout;
        tokio::spawn(async move {
            tokio_stream::wrappers::ReceiverStream::new(visit_url_rx)
                .for_each_concurrent(concurrent_tabs as usize, |crawl_request| {
                    let url = crawl_request.url();
                    let depth = crawl_request.depth();
                    debug!("crawling {} at depth {}", url, depth);

                    let at = active_tabs.clone();
                    let scraped_urls_tx = scraped_urls_tx.clone();
                    let failed_url_tx = failed_url_tx.clone();
                    let u = url.clone();
                    let base_url = base_url.clone();
                    let is_first_url = original_url == u;
                    let title_mutex = title.clone();

                    async move {
                        at.fetch_add(1, Ordering::SeqCst);

                        let links = task::spawn_blocking(move || {
                            // headless chrome can't handle pdfs, so we make a direct request for it
                            if u.as_str().ends_with(".pdf") {
                                match Self::fetch_pdf(u.clone()) {
                                    Ok(_) => return BrowsingResult::new(Vec::new(), None),
                                    Err(e) => return BrowsingResult::new(Vec::new(), Some(e.into())),
                                }
                            }

                            let browser = match BrowserController::new(timeout, min_wait, max_wait)
                            {
                                Ok(b) => b,
                                Err(e) => return BrowsingResult::new(Vec::new(), Some(e.into())),
                            };

                            let tab = browser.browse(u.as_str(), is_first_url);

                            if tab.is_err() {
                                let c = reqwest::blocking::Client::new();
                                let head = c.head(&u).send();

                                if head.is_err() {
                                    let err = head.err().unwrap();
                                    warn!(
                                        "error browsing for {} with tab err {}, head err {}",
                                        &u,
                                        tab.err().unwrap(),
                                        err
                                    );
                                    // we return an empty list of links, and flag as errored out
                                    return BrowsingResult::new(Vec::new(), Some(err.into()));
                                } else {
                                    let head = head.unwrap();
                                    let content_type = head.headers().get("Content-Type");
                                    if content_type.is_some()
                                        && content_type
                                            .unwrap()
                                            .to_str()
                                            .unwrap()
                                            .contains("application/pdf")
                                    {
                                        if Self::fetch_pdf(u.clone()).is_ok() {
                                            return BrowsingResult::new(Vec::new(), None);
                                        } else {
                                            return BrowsingResult::new(
                                                Vec::new(),
                                                Some(anyhow::anyhow!("could not fetch pdf").into()),
                                            );
                                        }
                                    } else {
                                        warn!(
                                            "error browsing for {} with tab err {}",
                                            &u,
                                            tab.err().unwrap(),
                                        );
                                        // we return an empty list of links, and flag as errored out
                                        return BrowsingResult::new(
                                            Vec::new(),
                                            Some(anyhow::anyhow!("not a pdf").into()),
                                        );
                                    }
                                }
                            }
                            let tab = tab.unwrap();
                            if is_first_url {
                                match tab.get_title() {
                                    Ok(title) => {
                                        let mut main_title = title_mutex.blocking_lock();
                                        *main_title = title;
                                    }
                                    Err(e) => {
                                        warn!("could not get title {:?}", e);
                                    }
                                };
                            }

                            return BrowsingResult::new(
                                browser
                                    .get_links(&tab)
                                    .iter()
                                    .filter_map(normalize_url_map(base_url.into()))
                                    .collect::<Vec<UrlInfo>>(),
                                None,
                            );
                        })
                        .await;

                        let links = match links {
                            Ok(l) => l,
                            Err(e) => {
                                error!("problem spawning a blocking thread {}", e);
                                at.fetch_sub(1, Ordering::SeqCst);
                                failed_url_tx
                                    .send(CrawlRequest::new(url, depth))
                                    .await
                                    .unwrap();
                                return;
                            }
                        };

                        // the boolean in the second element of the tuple
                        // tells us whether there was an error or not
                        // if so, we send the url to the failed url channel
                        if links.error.is_some() {
                            warn!(
                                "error browsing for {}, error: {}",
                                url,
                                links.error.unwrap()
                            );
                            match failed_url_tx.send(CrawlRequest::new(url, depth)).await {
                                Ok(_) => {}
                                Err(e) => {
                                    error!("could not send to failed_url_tx {}", e)
                                }
                            };
                        } else {
                            match scraped_urls_tx
                                .send(PageCrawlResult::new(url, links.links(), depth))
                                .await
                            {
                                Ok(_) => {}
                                Err(e) => {
                                    error!("could not send to scraped_urls_tx {}", e)
                                }
                            };
                        }
                        at.fetch_sub(1, Ordering::SeqCst);
                    }
                })
                .await;

            return;
        });
    }

    pub fn url(&self) -> String {
        self.url.to_string()
    }

    fn fetch_pdf(url: String) -> anyhow::Result<()> {
        match reqwest::blocking::get(url.as_str()) {
            Ok(res) => {
                debug!("fetching pdf at {}", url.as_str());
                let _r = res.text();
                return Ok(());
            }
            Err(e) => {
                warn!("error downloading pdf err: {}", e);
                return Err(anyhow::anyhow!(e));
            }
        }
    }
}
