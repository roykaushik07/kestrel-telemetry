use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Mqtt {
    pub broker: String,
    pub client_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Kafka {
    pub brokers: String,
    pub topic: String,
    pub linger_ms: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Dispatch {
    pub url: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Api {
    pub bind: String,
    #[serde(default = "default_window")]
    pub window_secs: u64,
}

fn default_window() -> u64 {
    300
}

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub mqtt: Mqtt,
    pub kafka: Kafka,
    pub dispatch: Dispatch,
    pub api: Api,
}

impl Settings {
    /// Layers config/default.toml, config/<KESTREL_ENV>.toml, then KESTREL__* env vars.
    pub fn load() -> Result<Self, config::ConfigError> {
        let env = std::env::var("KESTREL_ENV").unwrap_or_else(|_| "development".into());
        config::Config::builder()
            .add_source(config::File::with_name("config/default"))
            .add_source(config::File::with_name(&format!("config/{env}")).required(false))
            .add_source(config::Environment::with_prefix("KESTREL").separator("__"))
            .build()?
            .try_deserialize()
    }
}
