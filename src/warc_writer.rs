use rand::seq::SliceRandom;
use std::{
    ffi::OsStr,
    fs,
    io::{BufRead, BufReader},
    net,
    path::PathBuf,
    process::{self, Command, Stdio},
    sync::mpsc::sync_channel,
    thread::{self},
};
extern crate redis;
use rand::{thread_rng, Rng};
use redis::Commands;
pub struct WarcWriter {
    port: u16,
    process: std::process::Child,
    writer_dir: PathBuf,
    archive_name: String,
    persistent: bool,
}
use rand::distributions::Alphanumeric;

// Currently we use the wayback process to create our WARC file
impl WarcWriter {
    //
    pub fn new(
        port: Option<u16>,
        writer_dir: Option<PathBuf>,
        archive_name: Option<String>,
        persistent: bool,
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
        let writer_dir = if let Some(dir) = writer_dir {
            debug!("writer directory chosen {:?}", dir);
            dir
        } else {
            let d = create_random_tmp_folder()?;
            debug!("random writer directory created {:?}", d);
            d
        };

        init_wayback_config(&writer_dir)?;

        setup_dir(&archive_name, &writer_dir)?;

        let (tx, rx) = sync_channel(1);

        // purge the redis cache for our collection
        purge_redis(&archive_name)?;

        let port = if let Some(p) = port {
            p
        } else {
            get_available_port().unwrap()
        };
        let mut args: Vec<String> = vec![
            "--record".into(),
            "--live".into(),
            "-t 8".into(),
            format!("-p {}", port),
        ];
        debug!("{}", writer_dir.as_os_str().to_str().unwrap());
        let dir_str = writer_dir.as_os_str();
        if dir_str != OsStr::new("") && dir_str != OsStr::new(".") {
            args.push(format!("-d{}", writer_dir.as_os_str().to_str().unwrap()));
        }

        debug!("running wayback process with args: {:#?}", args);
        let mut process = Command::new("wayback")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;

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
                    // return;
                }
            }
        });

        while let Ok(mess) = rx.recv() {
            if mess == "ok" {
                break;
            } else {
                println!("Wayback error: {mess}");
                std::process::exit(1);
            }
        }

        Ok(WarcWriter {
            port,
            process,
            writer_dir,
            archive_name,
            persistent,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn writer_dir(&self) -> PathBuf {
        self.writer_dir.clone()
    }

    pub fn archive_name(&self) -> String {
        self.archive_name.clone()
    }

    pub fn terminate(&mut self) -> anyhow::Result<()> {
        if !self.persistent {
            // let mut d = self.writer_dir.clone();
            // d.push("collections");
            // debug!("{}", d.as_os_str().to_str().unwrap());
            // for entry in fs::read_dir(&d)? {
            //     debug!("{entry:?}");
            //     fs::remove_dir_all(entry?.path())?;
            // }
            // fs::remove_dir(&d)?;
            // d.pop();
            // fs::remove_dir(d)?;
        }
        debug!("Killing warc writer process with id {}", self.process.id());
        self.process.kill()?;
        Ok(())
    }
}

fn purge_redis(archive_name: &str) -> anyhow::Result<()> {
    // flush the redis cache in case we have the same name saved
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
    ports.iter().find(|port| port_is_available(**port)).cloned()
}

fn port_is_available(port: u16) -> bool {
    net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn setup_dir(archive_name: &str, writer_dir: &PathBuf) -> anyhow::Result<()> {
    // first check if we have a collection with wb-manager
    let mut dir = writer_dir.clone();
    dir.push("collections");
    dir.push(&archive_name);
    if !dir.exists() {
        let res = Command::new("wb-manager")
            .current_dir(writer_dir)
            .args(["init", archive_name])
            .status()?;

        if !res.success() {
            process::exit(res.code().unwrap());
        }
        let mut new_dir = dir.clone();
        new_dir.push("screenshots");

        fs::create_dir(new_dir)?;
    }
    Ok(())
}

fn create_random_tmp_folder() -> anyhow::Result<PathBuf> {
    let rand_folder_name: String = get_random_string(11);

    let path = PathBuf::from(format!("/tmp/archivoor-{}", rand_folder_name));
    fs::create_dir(&path)?;
    // populate this folder with a config.yaml
    Ok(path)
}

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
      filename_template: archivoor-{timestamp}-{hostname}-{random}.warc.gz
    "#;

    let mut p = path.clone();
    p.push("config.yaml");
    if p.exists() {
        debug!("config.yaml already exists, skipping");
        return Ok(());
    }

    fs::write(p, cfg)?;

    Ok(())
}

fn get_random_string(len: i32) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len as usize)
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn creates_a_random_folder() {
        // let path = ""
        let p = create_random_tmp_folder().unwrap();
        assert!(p.exists());
        fs::remove_dir(p).unwrap();
    }

    #[test]
    fn sets_up_collection() {
        let p = create_random_tmp_folder().unwrap();
        setup_dir("example".into(), &p).unwrap();
        fs::remove_dir_all(p).unwrap();
    }
}
