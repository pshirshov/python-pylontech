mod config;
mod error;
mod model;
mod mqtt;
mod protocol;
mod stats;

use std::thread;
use std::time::{Duration, Instant};

use clap::Parser;

use crate::config::{AppConfig, CliArgs};
use crate::error::{AppError, AppResult};
use crate::model::StackState;
use crate::mqtt::MqttPublisher;
use crate::protocol::PylontechClient;
use crate::stats::RuntimeStats;

fn main() -> Result<(), AppError> {
    let config = CliArgs::parse().into_config()?;
    run(config)
}

fn run(config: AppConfig) -> AppResult<()> {
    let stats = RuntimeStats::new_shared();
    stats.spawn_reporter(config.stats.interval);
    let mut reconnect_backoff =
        ReconnectBackoff::new(config.reconnect.initial_delay, config.reconnect.max_delay);

    loop {
        match run_session(&config, &stats) {
            Ok(()) => {
                reconnect_backoff.reset();
                return Ok(());
            }
            Err(error) => {
                stats.record_recovery();
                let delay = reconnect_backoff.next_delay();
                eprintln!("session failed: {}; reconnecting in {:?}", error, delay);
                thread::sleep(delay);
            }
        }
    }
}

fn run_session(config: &AppConfig, stats: &std::sync::Arc<RuntimeStats>) -> AppResult<()> {
    let publisher = MqttPublisher::connect(&config.mqtt, std::sync::Arc::clone(stats))?;
    let result = run_session_inner(config, &publisher, stats);
    if result.is_err() {
        publisher.publish_offline_best_effort();
    }
    result
}

fn run_session_inner(
    config: &AppConfig,
    publisher: &MqttPublisher,
    stats: &std::sync::Arc<RuntimeStats>,
) -> AppResult<()> {
    let mut client = PylontechClient::connect(&config.source, std::sync::Arc::clone(stats))?;
    let modules = client.scan_modules(config.polling.addresses())?;
    if modules.is_empty() {
        return Err(AppError::InvalidState(format!(
            "no modules discovered in range {}..={}",
            config.polling.scan_start, config.polling.scan_end
        )));
    }

    publisher.publish_discovery(&modules)?;
    eprintln!("published discovery for {} module(s)", modules.len());
    for module in &modules {
        let system_parameters = client.get_system_parameters(module.address)?;
        publisher.publish_system_parameters(module.address, &system_parameters)?;
    }

    let mut next_management_poll_at = Instant::now();
    loop {
        publisher.ensure_healthy()?;

        let now = Instant::now();
        if now >= next_management_poll_at {
            for module in &modules {
                let management_info = client.get_management_info(module.address)?;
                publisher.publish_management_info(module.address, &management_info)?;
            }
            next_management_poll_at = now + config.polling.management_interval;
        }

        let mut states = Vec::with_capacity(modules.len());
        for module in &modules {
            states.push(client.get_values_single(module.address)?);
        }

        let stack = StackState::from_modules(&states)?;
        publisher.publish_stack_state(&stack)?;
        for state in &states {
            publisher.publish_module_state(state)?;
        }
        stats.record_successful_cycle();

        thread::sleep(config.polling.interval);
    }
}

#[derive(Debug, Clone)]
struct ReconnectBackoff {
    current: Duration,
    initial: Duration,
    max: Duration,
}

impl ReconnectBackoff {
    fn new(initial: Duration, max: Duration) -> Self {
        Self {
            current: initial,
            initial,
            max,
        }
    }

    fn next_delay(&mut self) -> Duration {
        let delay = self.current;
        self.current = self.current.saturating_mul(2).min(self.max);
        delay
    }

    fn reset(&mut self) {
        self.current = self.initial;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::ReconnectBackoff;

    #[test]
    fn reconnect_backoff_doubles_and_caps() {
        let mut backoff = ReconnectBackoff::new(Duration::from_secs(1), Duration::from_secs(5));

        assert_eq!(backoff.next_delay(), Duration::from_secs(1));
        assert_eq!(backoff.next_delay(), Duration::from_secs(2));
        assert_eq!(backoff.next_delay(), Duration::from_secs(4));
        assert_eq!(backoff.next_delay(), Duration::from_secs(5));
        assert_eq!(backoff.next_delay(), Duration::from_secs(5));

        backoff.reset();
        assert_eq!(backoff.next_delay(), Duration::from_secs(1));
    }
}
