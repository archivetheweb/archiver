use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::sync_channel;
use std::thread;

pub struct Writer {
    pub port: u16,
    debug: bool,
    process: std::process::Child,
}

// Currently we use the wayback process to create our WARC file
impl Writer {
    pub fn new(port: u16, debug: bool) -> anyhow::Result<Self> {
        let (tx, rx) = sync_channel(1);

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

        let res = BufReader::new(stderr).lines();

        let tx1 = tx.clone();

        thread::spawn(move || {
            for line in res {
                if debug {
                    println!("{line:?}");
                }
                let l = line.unwrap();
                if l.contains("Starting Gevent Server on") {
                    tx1.send("ok".to_string()).unwrap();
                } else if l.contains("Traceback") {
                    tx1.send(l).unwrap();
                }
            }
        });

        for mess in rx.recv() {
            if mess == "ok" {
                break;
            } else {
                println!("Wayback error: {mess}");
                std::process::exit(1);
            }
        }

        Ok(Writer {
            port,
            debug,
            process,
        })
    }

    pub fn terminate(mut self) -> anyhow::Result<()> {
        self.process.kill()?;
        Ok(())
    }
}
