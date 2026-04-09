use std::fs;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;

use crate::error::{AppError, AppResult};

const DEFAULT_SOURCE_PORT: u16 = 23;
const DEFAULT_MQTT_PORT: u16 = 1883;
const DEFAULT_TIMEOUT_MILLIS: u64 = 2_000;
const DEFAULT_INTERVAL_MILLIS: u64 = 1_000;
const DEFAULT_MANAGEMENT_INTERVAL_MILLIS: u64 = 30_000;
const DEFAULT_RECONNECT_INITIAL_DELAY_MILLIS: u64 = 1_000;
const DEFAULT_RECONNECT_MAX_DELAY_MILLIS: u64 = 30_000;
const DEFAULT_STATS_INTERVAL_MILLIS: u64 = 300_000;
const DEFAULT_SCAN_START: u8 = 2;
const DEFAULT_SCAN_END: u8 = 9;
const DEFAULT_DISCOVERY_PREFIX: &str = "homeassistant";
const DEFAULT_TOPIC_PREFIX: &str = "pylontech";
const DEFAULT_CLIENT_ID: &str = "pylontech-rs-mqtt-adapter";

#[derive(Debug, Parser)]
#[command(about = "Standalone Rust MQTT adapter for Pylontech batteries")]
pub struct CliArgs {
    #[arg(help = "TCP bridge host")]
    pub source_host: String,
    #[arg(long, default_value_t = DEFAULT_SOURCE_PORT, help = "TCP bridge port")]
    pub source_port: u16,
    #[arg(long, default_value_t = DEFAULT_TIMEOUT_MILLIS, help = "Read/write timeout in milliseconds")]
    pub timeout_millis: u64,
    #[arg(long, default_value_t = DEFAULT_INTERVAL_MILLIS, help = "Polling interval in milliseconds")]
    pub interval_millis: u64,
    #[arg(long, default_value_t = DEFAULT_MANAGEMENT_INTERVAL_MILLIS, help = "Management info polling interval in milliseconds")]
    pub management_interval_millis: u64,
    #[arg(long, default_value_t = DEFAULT_SCAN_START, help = "First module address to probe, inclusive")]
    pub scan_start: u8,
    #[arg(long, default_value_t = DEFAULT_SCAN_END, help = "Last module address to probe, inclusive")]
    pub scan_end: u8,
    #[arg(long, default_value_t = DEFAULT_RECONNECT_INITIAL_DELAY_MILLIS, help = "Initial reconnect delay in milliseconds")]
    pub reconnect_initial_delay_millis: u64,
    #[arg(long, default_value_t = DEFAULT_RECONNECT_MAX_DELAY_MILLIS, help = "Maximum reconnect delay in milliseconds")]
    pub reconnect_max_delay_millis: u64,
    #[arg(long, default_value_t = DEFAULT_STATS_INTERVAL_MILLIS, help = "Periodic liveness stats interval in milliseconds")]
    pub stats_interval_millis: u64,
    #[arg(long, help = "MQTT broker host")]
    pub mqtt_host: String,
    #[arg(long, default_value_t = DEFAULT_MQTT_PORT, help = "MQTT broker port")]
    pub mqtt_port: u16,
    #[arg(long, help = "MQTT username")]
    pub mqtt_user: Option<String>,
    #[arg(long, help = "MQTT password")]
    pub mqtt_password: Option<String>,
    #[arg(long, help = "MQTT password file")]
    pub mqtt_password_file: Option<PathBuf>,
    #[arg(long, default_value = DEFAULT_DISCOVERY_PREFIX, help = "Home Assistant MQTT discovery prefix")]
    pub discovery_prefix: String,
    #[arg(long, default_value = DEFAULT_TOPIC_PREFIX, help = "Base topic prefix for state and availability topics")]
    pub topic_prefix: String,
    #[arg(long, default_value = DEFAULT_CLIENT_ID, help = "MQTT client id")]
    pub client_id: String,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub source: SourceConfig,
    pub polling: PollingConfig,
    pub mqtt: MqttConfig,
    pub reconnect: ReconnectConfig,
    pub stats: StatsConfig,
}

#[derive(Debug, Clone)]
pub struct SourceConfig {
    pub host: String,
    pub port: u16,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct PollingConfig {
    pub interval: Duration,
    pub management_interval: Duration,
    pub scan_start: u8,
    pub scan_end: u8,
}

impl PollingConfig {
    pub fn addresses(&self) -> RangeInclusive<u8> {
        self.scan_start..=self.scan_end
    }
}

#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub discovery_prefix: String,
    pub topic_prefix: String,
    pub client_id: String,
}

#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

#[derive(Debug, Clone)]
pub struct StatsConfig {
    pub interval: Duration,
}

impl CliArgs {
    pub fn into_config(self) -> AppResult<AppConfig> {
        if self.source_host.trim().is_empty() {
            return Err(AppError::InvalidConfig(
                "source host cannot be empty".to_string(),
            ));
        }
        if self.mqtt_host.trim().is_empty() {
            return Err(AppError::InvalidConfig(
                "mqtt host cannot be empty".to_string(),
            ));
        }
        if self.timeout_millis == 0 {
            return Err(AppError::InvalidConfig(
                "timeout must be greater than zero".to_string(),
            ));
        }
        if self.interval_millis == 0 {
            return Err(AppError::InvalidConfig(
                "interval must be greater than zero".to_string(),
            ));
        }
        if self.management_interval_millis == 0 {
            return Err(AppError::InvalidConfig(
                "management interval must be greater than zero".to_string(),
            ));
        }
        if self.scan_start > self.scan_end {
            return Err(AppError::InvalidConfig(format!(
                "scan start {} cannot be greater than scan end {}",
                self.scan_start, self.scan_end
            )));
        }
        if self.reconnect_initial_delay_millis == 0 {
            return Err(AppError::InvalidConfig(
                "reconnect initial delay must be greater than zero".to_string(),
            ));
        }
        if self.reconnect_max_delay_millis == 0 {
            return Err(AppError::InvalidConfig(
                "reconnect max delay must be greater than zero".to_string(),
            ));
        }
        if self.reconnect_initial_delay_millis > self.reconnect_max_delay_millis {
            return Err(AppError::InvalidConfig(format!(
                "reconnect initial delay {} cannot be greater than reconnect max delay {}",
                self.reconnect_initial_delay_millis, self.reconnect_max_delay_millis
            )));
        }
        if self.stats_interval_millis == 0 {
            return Err(AppError::InvalidConfig(
                "stats interval must be greater than zero".to_string(),
            ));
        }

        let password = load_password(self.mqtt_password, self.mqtt_password_file)?;
        if password.is_some() && self.mqtt_user.is_none() {
            return Err(AppError::InvalidConfig(
                "mqtt password requires --mqtt-user".to_string(),
            ));
        }

        Ok(AppConfig {
            source: SourceConfig {
                host: self.source_host.trim().to_string(),
                port: self.source_port,
                timeout: Duration::from_millis(self.timeout_millis),
            },
            polling: PollingConfig {
                interval: Duration::from_millis(self.interval_millis),
                management_interval: Duration::from_millis(self.management_interval_millis),
                scan_start: self.scan_start,
                scan_end: self.scan_end,
            },
            mqtt: MqttConfig {
                host: self.mqtt_host.trim().to_string(),
                port: self.mqtt_port,
                username: self.mqtt_user.map(|value| value.trim().to_string()),
                password,
                discovery_prefix: normalize_topic_segment(
                    &self.discovery_prefix,
                    "discovery prefix",
                )?,
                topic_prefix: normalize_topic_segment(&self.topic_prefix, "topic prefix")?,
                client_id: self.client_id.trim().to_string(),
            },
            reconnect: ReconnectConfig {
                initial_delay: Duration::from_millis(self.reconnect_initial_delay_millis),
                max_delay: Duration::from_millis(self.reconnect_max_delay_millis),
            },
            stats: StatsConfig {
                interval: Duration::from_millis(self.stats_interval_millis),
            },
        })
    }
}

fn normalize_topic_segment(value: &str, field_name: &str) -> AppResult<String> {
    let normalized = value.trim().trim_matches('/').to_string();
    if normalized.is_empty() {
        return Err(AppError::InvalidConfig(format!(
            "{field_name} cannot be empty"
        )));
    }
    Ok(normalized)
}

fn load_password(
    password: Option<String>,
    password_file: Option<PathBuf>,
) -> AppResult<Option<String>> {
    match (password, password_file) {
        (Some(_), Some(_)) => Err(AppError::InvalidConfig(
            "use either --mqtt-password or --mqtt-password-file".to_string(),
        )),
        (Some(password), None) => Ok(Some(password)),
        (None, Some(path)) => {
            let password = fs::read_to_string(path)?;
            let password = password.trim().to_string();
            if password.is_empty() {
                return Err(AppError::InvalidConfig(
                    "mqtt password file was empty".to_string(),
                ));
            }
            Ok(Some(password))
        }
        (None, None) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::CliArgs;

    #[test]
    fn into_config_rejects_reconnect_initial_delay_above_max() {
        let args = CliArgs {
            source_host: "battery.local".to_string(),
            source_port: 23,
            timeout_millis: 2_000,
            interval_millis: 1_000,
            management_interval_millis: 30_000,
            scan_start: 2,
            scan_end: 9,
            reconnect_initial_delay_millis: 10_000,
            reconnect_max_delay_millis: 1_000,
            stats_interval_millis: 300_000,
            mqtt_host: "mqtt.local".to_string(),
            mqtt_port: 1883,
            mqtt_user: None,
            mqtt_password: None,
            mqtt_password_file: None,
            discovery_prefix: "homeassistant".to_string(),
            topic_prefix: "pylontech".to_string(),
            client_id: "client".to_string(),
        };

        let error = args.into_config().unwrap_err();
        assert!(error.to_string().contains(
            "reconnect initial delay 10000 cannot be greater than reconnect max delay 1000"
        ));
    }
}
