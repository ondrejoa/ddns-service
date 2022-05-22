use crate::{Config, storage::WatchCache, ShutdownMsg};
use anyhow::Result;
use log::{error, info};
use reqwest::Client;
use std::net::Ipv4Addr;
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::time::interval;

pub struct Watch {
    client: Client,
    tx: Sender<Ipv4Addr>,
    shutdown: Sender<ShutdownMsg>,
    interval: u64,
    cache: WatchCache,
}

impl Watch {
    pub fn new(
        cache: &Path,
        conf: &Config,
        shutdown: Sender<ShutdownMsg>,
    ) -> (Self, Receiver<Ipv4Addr>) {
        let (tx, rx) = mpsc::channel(1);
        let client = Client::new();
        (
            Self {
                client,
                tx,
                shutdown,
                interval: conf.interval,
                cache: WatchCache::new(cache),
            },
            rx,
        )
    }

    pub async fn run(mut self) {
        let mut interval = interval(Duration::new(self.interval, 0));
        interval.tick().await;
        loop {
            match get_ipv4(&mut self.client).await {
                Ok(ip) => {
                    match self.cache.put(ip) {
                        Ok(Some(ip)) => {
                            info!("Ip changed: {}", ip);
                            let _ = self.tx.send(ip).await;
                        },
                        Ok(_) => {
                            info!("No ip change detected");
                        }
                        Err(e) => {
                            error!("Can't open cache.yaml: {}", e.to_string());
                            let _ = self.shutdown.send(ShutdownMsg).await;
                            return;
                        }
                    }
                }
                Err(e) => {
                    error!("Can't get ip: {}", e.to_string());
                    let _ = self.shutdown.send(ShutdownMsg).await;
                    return;
                }
            };
            interval.tick().await;
        }
    }
}


async fn get_ipv4(client: &mut reqwest::Client) -> Result<Ipv4Addr> {
    Ok(client
        .get("https://ifconfig.me")
        .send()
        .await?
        .text()
        .await?
        .trim()
        .parse()?)
}
