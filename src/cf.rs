use crate::{Config, ShutdownMsg};
use anyhow::{Context, Result};
use cloudflare::endpoints::dns::{
    DnsContent, ListDnsRecords, ListDnsRecordsParams, UpdateDnsRecord, UpdateDnsRecordParams,
};
use cloudflare::endpoints::zone::{ListZones, ListZonesParams};
use cloudflare::framework::auth::Credentials;
use cloudflare::framework::{async_api::Client, Environment, HttpApiClientConfig};
use log::{debug, error, info, warn};
use std::net::Ipv4Addr;
use tokio::sync::mpsc::{Receiver, Sender};

pub struct DnsUpdater {
    client: Client,
    ipv4: bool,
    zone: String,
    domains: Vec<String>,
    rx: Receiver<Ipv4Addr>,
    shutdown: Sender<ShutdownMsg>,
}

impl DnsUpdater {
    pub async fn new(
        conf: &Config,
        rx: Receiver<Ipv4Addr>,
        shutdown: Sender<ShutdownMsg>,
    ) -> Result<Self> {
        let mut client = Client::new(
            Credentials::UserAuthToken {
                token: conf.token.clone(),
            },
            HttpApiClientConfig::default(),
            Environment::Production,
        )?;
        let zone = get_zone(&conf.zone, &mut client).await?;
        info!("Zone: {}, Id: {}", conf.zone, zone);
        Ok(Self {
            client,
            ipv4: conf.ipv4(),
            zone,
            domains: conf.domains.clone(),
            rx,
            shutdown,
        })
    }

    pub async fn run(mut self) {
        loop {
            debug!("Waiting for ip changes");
            if let Some(ip) = self.rx.recv().await {
                if self.ipv4 {
                    for domain in self.domains.iter() {
                        if let Ok(rid) = get_record(
                            &self.zone,
                            domain,
                            DnsContent::A {
                                content: Ipv4Addr::UNSPECIFIED,
                            },
                            &mut self.client,
                        )
                        .await
                        {
                            debug!("Updating domain: {}, Rid: {}", domain, rid);
                            if let Err(e) = update_record(
                                &self.zone,
                                &rid,
                                &domain,
                                DnsContent::A { content: ip },
                                &mut self.client,
                            ).await {
                                error!("Updating domain: {}, {}", domain, e.to_string());
                                let _ = self.shutdown.send(ShutdownMsg).await;
                                return;
                            }
                        } else {
                            warn!("Can't get record of domain: {}", domain);
                        }
                    }
                }
            }
        }
    }
}
async fn get_zone(domain: &str, client: &mut Client) -> Result<String> {
    let zones = client
        .request_handle(&ListZones {
            params: ListZonesParams {
                name: Some(domain.to_owned()),
                status: None,
                page: None,
                per_page: None,
                order: None,
                direction: None,
                search_match: None,
            },
        })
        .await?
        .result;
    Ok(zones[0].id.clone())
}

async fn get_record(
    zone_identifier: &str,
    domain: &str,
    r#type: DnsContent,
    client: &mut Client,
) -> Result<String> {
    Ok(client
        .request_handle(&ListDnsRecords {
            zone_identifier,
            params: ListDnsRecordsParams {
                record_type: None,
                name: Some(domain.to_owned()),
                page: None,
                per_page: None,
                order: None,
                direction: None,
                search_match: None,
            },
        })
        .await
        .context("Couldn't fetch record")?
        .result
        .iter()
        .find(|record| std::mem::discriminant(&record.content) == std::mem::discriminant(&r#type))
        .context("No matching record found")?
        .id
        .clone())
}

async fn update_record(
    zone_identifier: &str,
    identifier: &str,
    name: &str,
    content: DnsContent,
    client: &Client,
) -> Result<()> {
    client
        .request_handle(&UpdateDnsRecord {
            zone_identifier,
            identifier,
            params: UpdateDnsRecordParams {
                ttl: None,
                proxied: None,
                name,
                content,
            },
        })
        .await?;
    Ok(())
}
