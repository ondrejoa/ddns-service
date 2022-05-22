use crate::ip_watch::Watch;
use crate::storage::*;
use anyhow::Result;
use log::info;
use tokio::signal::ctrl_c;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;
use tokio::spawn;
use crate::cf::DnsUpdater;

extern crate log;

mod ip_watch;
mod storage;
mod cf;

pub struct ShutdownMsg;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let files = Files::new()?;
    let conf = Config::new(&files)?;

    info!("Update interval: {} seconds", conf.interval);

    let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);

    let (watch, rx) = Watch::new(&files.data, &conf, shutdown_tx.clone());
    let updater = DnsUpdater::new(&conf, rx, shutdown_tx.clone()).await?;

    spawn(watch.run());
    spawn(updater.run());

    drop(shutdown_tx);

    let mut sigterm = signal(SignalKind::terminate())?;
    tokio::select! {
         _ = ctrl_c() => {},
         _ = sigterm.recv() => {},
         Some(_) = shutdown_rx.recv() => {},
    };

    info!("Exiting");

    Ok(())
}



