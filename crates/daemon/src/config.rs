use anyhow::{bail, Result};
use crossdesk_core::*;
use crossdesk_transport::{atomic_save, data_dir, PairLayout};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub peer: String,
    pub address: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_configuration_defaults_to_subnet_restriction_and_choice_persists() {
        let mut json = serde_json::to_value(Config::default()).unwrap();
        json.as_object_mut().unwrap().remove("same_subnet_only");
        let mut config: Config = serde_json::from_value(json).unwrap();
        assert!(config.same_subnet_only);
        config.same_subnet_only = false;
        let restored: Config =
            serde_json::from_slice(&serde_json::to_vec(&config).unwrap()).unwrap();
        assert!(!restored.same_subnet_only);
    }
    #[test]
    fn listening_port_reserves_the_next_port_for_pairing() {
        for port in [0, 1023, 65535] {
            assert!(Config {
                port,
                ..Default::default()
            }
            .validate()
            .is_err());
        }
        for port in [1024, 24872, 65534] {
            assert!(Config {
                port,
                ..Default::default()
            }
            .validate()
            .is_ok());
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub sharing: bool,
    #[serde(default = "default_same_subnet")]
    pub same_subnet_only: bool,
    pub version: u32,
    pub port: u16,
    pub endpoints: Vec<Endpoint>,
    pub links: Vec<EdgeLink>,
    #[serde(default)]
    pub layout_revisions: std::collections::BTreeMap<String, PairLayout>,
    #[serde(default)]
    pub tuning: Tuning,
}
fn default_same_subnet() -> bool {
    true
}
impl Default for Config {
    fn default() -> Self {
        Self {
            sharing: false,
            same_subnet_only: true,
            version: 1,
            port: crossdesk_core::profile::DEFAULT_PORT,
            endpoints: vec![],
            links: vec![],
            layout_revisions: Default::default(),
            tuning: Tuning::default(),
        }
    }
}
impl Config {
    pub fn load() -> Result<Self> {
        let p = data_dir().join("config.json");
        let c = if p.exists() {
            serde_json::from_slice(&std::fs::read(p)?)?
        } else {
            Self::default()
        };
        Ok(c)
    }
    pub fn validate(&self) -> Result<()> {
        self.tuning.validate().map_err(anyhow::Error::msg)?;
        if self.version != 1 || !(1024..=65534).contains(&self.port) {
            bail!("unsupported configuration version or port");
        }
        if self.endpoints.len() > 8 || self.links.len() > 64 {
            bail!("too many peers or links");
        }
        let mut identities = std::collections::BTreeSet::new();
        for e in &self.endpoints {
            if !identities.insert(&e.peer) {
                bail!("duplicate endpoint identity");
            }
            let _: std::net::SocketAddr = e.address.parse()?;
        }
        Ok(())
    }
    pub fn save(&self) -> Result<()> {
        self.validate()?;
        atomic_save(
            &data_dir().join("config.json"),
            &serde_json::to_vec_pretty(self)?,
        )
    }
}
