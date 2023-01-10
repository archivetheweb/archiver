use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::sync_channel;
use std::thread;
use sysinfo::{ProcessExt, System, SystemExt};
extern crate redis;
use redis::Commands;
pub struct WarcWriter {
    port: u16,
    process: std::process::Child,
}

// Currently we use the wayback process to create our WARC file
impl WarcWriter {
    pub fn new(port: u16, debug: bool) -> anyhow::Result<Self> {
        let (tx, rx) = sync_channel(1);

        let s = System::new_all();

        // we kill the processes first
        for process in s.processes_by_exact_name("wayback") {
            process.kill();
        }

        // flush the redis cache
        let client = redis::Client::open("redis://127.0.0.1/")?;
        let mut con = client.get_connection()?;
        let pending_index: i32 = con.del("pywb:archivoor:pending")?;
        let index: i32 = con.del("pywb:archivoor:cdxj")?;
        if pending_index > 0 {
            debug!("pending index deleted from redis");
        }
        if index > 0 {
            debug!("index deleted from redis");
        }

        // then run it
        let mut process = Command::new("wayback")
            .args([
                "--record",
                "--live",
                "-t 8",
                format!("-p {}", port).as_ref(),
            ])
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
                    return;
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

        Ok(WarcWriter { port, process })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn terminate(&mut self) -> anyhow::Result<()> {
        debug!("Killing warc writer process with id {}", self.process.id());
        self.process.kill()?;
        Ok(())
    }
}
