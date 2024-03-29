use rand::seq::SliceRandom;
use rand::thread_rng;
use std::{
    ffi::OsStr,
    fs::{self, DirEntry},
    io::{BufRead, BufReader},
    net,
    path::PathBuf,
    process::{self, Command, Stdio},
    sync::mpsc::sync_channel,
    thread::{self},
};

use urlencoding::encode;
extern crate redis;
use anyhow::{anyhow, Context};
use redis::Commands;
use sysinfo::{PidExt, System, SystemExt};

use crate::utils::{create_random_tmp_folder, get_random_string, get_tmp_screenshot_dir};

pub struct WarcWriter {
    port: u16,
    process: std::process::Child,
    archive_dir: PathBuf,
    archive_name: String,
}

impl WarcWriter {
    pub fn new(
        port: Option<u16>,
        parent_dir: Option<PathBuf>,
        archive_name: Option<String>,
        debug: bool,
    ) -> anyhow::Result<Self> {
        let archive_name = if let Some(n) = archive_name {
            debug!("archive name chosen: {}", n);
            n
        } else {
            let n = get_random_string(11);
            debug!("random archive name: {}", n);
            n
        };

        // first we check if we have the write folder structure
        let parent_dir = if let Some(dir) = parent_dir {
            debug!("writer directory chosen {:?}", dir);
            dir
        } else {
            let d = create_random_tmp_folder()?;
            debug!("random writer directory created {:?}", d);
            d
        };

        Self::init_wayback_config(&parent_dir).context("could not initialize wayback configs")?;

        Self::setup_dir(&archive_name, &parent_dir)
            .context("could not setup necessary directories")?;

        let (tx, rx) = sync_channel(1);

        // purge the redis cache for our collection
        Self::purge_redis(&archive_name).context("could not purge redis cache")?;

        let port = if let Some(p) = port {
            p
        } else {
            Self::get_available_port().unwrap()
        };
        let mut args: Vec<String> = vec![
            "--record".into(),
            "--live".into(),
            "-t 8".into(),
            format!("-p {}", port),
        ];
        debug!("{}", parent_dir.as_os_str().to_str().unwrap());
        let dir_str = parent_dir.as_os_str();
        if dir_str != OsStr::new("") && dir_str != OsStr::new(".") {
            args.push(format!("-d{}", parent_dir.as_os_str().to_str().unwrap()));
        }

        debug!("running wayback process with args: {:#?}", args);
        let mut process = Command::new("wayback")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .context("could not spawn wayback process")?;

        let stderr = process.stderr.take().unwrap();

        thread::spawn(move || {
            let res = BufReader::new(stderr).lines();
            let tx = tx.clone();
            for line in res {
                if debug {
                    println!("{line:?}");
                }
                let l = line.unwrap();
                if l.contains("Starting Gevent Server on") {
                    debug!("wayback proxy spawned successfully");
                    tx.send("ok".to_string()).unwrap();
                    if !debug {
                        return;
                    }
                } else if l.contains("Traceback") || l.contains("usage: wayback") {
                    error!("error spawning wayback proxy");
                    match tx.send(l) {
                        Ok(_) => {}
                        Err(e) => {
                            warn!("error sending message to wayback thread: {}", e)
                        }
                    }
                }
            }
        });

        while let Ok(mess) = rx.recv() {
            if mess == "ok" {
                break;
            } else {
                println!("wayback error: {mess}");
                std::process::exit(1);
            }
        }

        let s = System::new_all();
        if let None = s.process(PidExt::from_u32(process.id())) {
            return Err(anyhow!("wayback error: process is not running"));
        }

        let mut archive_dir = parent_dir.clone();
        archive_dir.push("collections");
        archive_dir.push(archive_name.clone());
        archive_dir.push("archive");

        Ok(WarcWriter {
            port,
            process,
            archive_dir,
            archive_name,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn archive_dir(&self) -> PathBuf {
        self.archive_dir.clone()
    }

    pub fn archive_name(&self) -> String {
        self.archive_name.clone()
    }

    fn fetch_all_warcs(&self) -> anyhow::Result<Vec<DirEntry>> {
        let dir = fs::read_dir(self.archive_dir())
            .context(format!("could not read_dir {:?}", self.archive_dir()))?;

        let dir: Vec<DirEntry> = dir
            .into_iter()
            .filter_map(|x| match x {
                Ok(x) => {
                    if x.file_name().to_str().unwrap().contains(".warc") {
                        return Some(x);
                    } else {
                        return None;
                    }
                }
                Err(e) => {
                    error!("error reading warcs dir {}", e);
                    None
                }
            })
            .collect();
        Ok(dir)
    }

    pub fn rename_warc_files(&self, new_name: &str, depth: i32) -> anyhow::Result<Vec<PathBuf>> {
        let warcs = self.fetch_all_warcs()?;

        let filenames = warcs
            .iter()
            .filter_map(|x| {
                let file_name = x.file_name();
                let file_name = file_name.to_str().unwrap();
                if file_name.contains("<unprocessed>") {
                    let name_elems: Vec<&str> = file_name.trim().split("-").collect();

                    // The timestamp on the document is as follows, we remove the
                    // microseconds from the timestamp. Time is in UTC
                    // 20230125 160157 993364
                    // YYYYMMDD HHMMSS Microseconds

                    let ts = name_elems[2].clone().split_at(14).0;
                    // the name matters as we will be using it to
                    let new_full_name =
                        format!("archiver_{}_{}_{}.warc.gz", ts, encode(new_name), depth);
                    let mut new_path = x.path().clone();
                    new_path.pop();
                    new_path.push(&new_full_name);

                    match fs::rename(x.path(), &new_path) {
                        Ok(_) => {
                            debug!("renamed {} to {}", file_name, new_full_name);
                            return Some(new_path);
                        }
                        Err(e) => {
                            error!("could not rename {} with err: {}", file_name, e);
                            return None;
                        }
                    }
                }
                None
            })
            .collect::<Vec<PathBuf>>();

        Ok(filenames)
    }

    pub fn process_screenshot(
        &self,
        ts: &str,
        domain: &str,
        depth: i32,
    ) -> anyhow::Result<PathBuf> {
        let mut dir = self.archive_dir.clone();
        dir.pop();
        dir.push("screenshots");
        dir.push(format!("archiver_{}_{}_{}.png", ts, encode(domain), depth));
        let screenshot_dir = get_tmp_screenshot_dir(&self.archive_name);
        fs::copy(&screenshot_dir, &dir).context(format!(
            "could not copy screenshot from {:?} to {:?}",
            &screenshot_dir, &dir
        ))?;
        fs::remove_file(&screenshot_dir)
            .context(format!("could not remove file {}", screenshot_dir))?;
        Ok(dir)
    }

    pub fn terminate(&mut self) -> anyhow::Result<()> {
        debug!("killing warc writer process with id {}", self.process.id());
        self.process.kill()?;
        Ok(())
    }

    // flushes the redis cache to avoid stale data
    fn purge_redis(archive_name: &str) -> anyhow::Result<()> {
        let client = redis::Client::open("redis://127.0.0.1/")?;
        let mut con = client.get_connection()?;
        let pending_index: i32 = con.del(format!("pywb:{}:pending", archive_name))?;
        let index: i32 = con.del(format!("pywb:{}:cdxj", archive_name))?;
        match (pending_index, index) {
            (0, 0) => {
                debug!("nothing to purge for {}", archive_name)
            }
            (x, y) if x > 0 && y > 0 => {
                debug!("purged both pending and index cache for {}", archive_name)
            }
            (x, _) if x > 0 => {
                debug!("pending purged for {}", archive_name)
            }
            (_, y) if y > 0 => {
                debug!("index purged for {}", archive_name)
            }
            _ => {}
        }
        if pending_index > 0 {
            debug!("pending index deleted from redis");
        }
        if index > 0 {
            debug!("index deleted from redis");
        }
        Ok(())
    }

    fn get_available_port() -> Option<u16> {
        let mut ports: Vec<u16> = (8000..9000).collect();
        ports.shuffle(&mut thread_rng());
        ports
            .iter()
            .find(|port| Self::port_is_available(**port))
            .cloned()
    }

    fn port_is_available(port: u16) -> bool {
        net::TcpListener::bind(("127.0.0.1", port)).is_ok()
    }

    fn setup_dir(archive_name: &str, parent_dir: &PathBuf) -> anyhow::Result<()> {
        // first check if we have a collection with wb-manager
        let mut dir = parent_dir.clone();
        dir.push("collections");
        dir.push(&archive_name);
        if !dir.exists() {
            let res = Command::new("wb-manager")
                .current_dir(parent_dir)
                .args(["init", archive_name])
                .status()
                .context("could not setup dir using wb-manager")?;

            if !res.success() {
                process::exit(res.code().unwrap());
            }
            let mut new_dir = dir.clone();
            new_dir.push("screenshots");

            fs::create_dir(&new_dir).context(format!("could not create dir {:?}", new_dir))?;
        }
        Ok(())
    }

    // Wayback config necessary for the application to work as desired
    fn init_wayback_config(path: &PathBuf) -> anyhow::Result<()> {
        let cfg = r#"
    collections_root: collections
    archive_paths: archive
    index_paths: indexes
    static_path: static
    templates_dir: templates
    
    framed_replay: false
    
    recorder:
      dedup_policy: skip
      dedup_index_url: "redis://localhost:6379/0/pywb:{coll}:cdxj"
      source_coll: live
      filename_template: <unprocessed>-archiver-{timestamp}-{random}.warc.gz
    "#;

        let mut p = path.clone();
        p.push("config.yaml");
        if p.exists() {
            debug!("config.yaml already exists, skipping");
            return Ok(());
        }

        fs::write(p, cfg).context(format!("could not create config.yaml"))?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::utils::create_random_tmp_folder;

    use super::*;

    #[test]
    fn sets_up_collection() {
        let p = create_random_tmp_folder().unwrap();
        WarcWriter::setup_dir("example".into(), &p).unwrap();
        fs::remove_dir_all(p).unwrap();
    }
}
