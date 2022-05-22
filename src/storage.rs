use anyhow::{Context, Result};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::fs::File;

#[derive(Debug)]
pub struct Files {
    pub config: PathBuf,
    pub data: PathBuf,
}

impl Files {
    pub fn new() -> Result<Self> {
        let mut config = if let Some(conf) = option_env!("CONF_DIR") {
            PathBuf::from(conf)
        } else {
            let basedirs = BaseDirs::new().context("Can't get basedirs")?;
            PathBuf::from(basedirs.config_dir())
        };
        config.push("config.yaml");
        let mut data = if let Some(data) = option_env!("DATA_DIR") {
            PathBuf::from(data)
        } else {
            let basedirs = BaseDirs::new().context("Can't get basedirs")?;
            PathBuf::from(basedirs.data_dir())
        };
        data.push("cache.yaml");
        Ok(Self { config, data })
    }
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub token: String,
    pub zone: String,
    pub domains: Vec<String>,
    records: Vec<String>,
    pub interval: u64,
}

impl Config {
    pub fn new(files: &Files) -> Result<Self> {
        let config = File::open(&files.config).context("config.yaml")?;
        Ok(serde_yaml::from_reader(config)?)
    }

    pub fn ipv4(&self) -> bool {
        self.records.iter().filter(|s| *s == "A").count() != 0
    }
}

#[derive(Serialize, Deserialize)]
pub struct WatchCache {
    ipv4: Ipv4Addr,
    #[serde(skip)]
    cache_file: PathBuf,
}

impl WatchCache {
    pub fn new(f: &Path) -> Self {
        let f = f.to_path_buf();
        match File::open(f.clone()) {
            Ok(cache_file) => {
                let mut cache = serde_yaml::from_reader(cache_file).unwrap_or(Self {
                    ipv4: Ipv4Addr::UNSPECIFIED,
                    cache_file: Default::default(),
                });
                cache.cache_file = f.to_path_buf();
                cache
            }
            _ => Self {
                ipv4: Ipv4Addr::UNSPECIFIED,
                cache_file: f.to_path_buf(),
            },
        }
    }

    pub fn put(&mut self, ipv4: Ipv4Addr) -> Result<Option<Ipv4Addr>> {
        if ipv4 != self.ipv4 {
            self.ipv4 = ipv4;
            let cache_file = File::create(self.cache_file.clone())?;
            serde_yaml::to_writer(cache_file, &self)?;
            Ok(Some(ipv4))
        } else {
            Ok(None)
        }
    }
}
