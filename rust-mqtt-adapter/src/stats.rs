use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct RuntimeStats {
    started_at: Instant,
    successful_cycles: AtomicU64,
    mqtt_messages_sent: AtomicU64,
    recoveries: AtomicU64,
    source_bytes_read: AtomicU64,
    source_bytes_written: AtomicU64,
    source_frames_read: AtomicU64,
    source_frames_written: AtomicU64,
    source_timeouts: AtomicU64,
    source_errors: AtomicU64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsSnapshot {
    pub uptime: Duration,
    pub successful_cycles: u64,
    pub mqtt_messages_sent: u64,
    pub recoveries: u64,
    pub source_bytes_read: u64,
    pub source_bytes_written: u64,
    pub source_frames_read: u64,
    pub source_frames_written: u64,
    pub source_timeouts: u64,
    pub source_errors: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsDelta {
    pub successful_cycles: u64,
    pub mqtt_messages_sent: u64,
    pub recoveries: u64,
    pub source_bytes_read: u64,
    pub source_bytes_written: u64,
    pub source_frames_read: u64,
    pub source_frames_written: u64,
    pub source_timeouts: u64,
    pub source_errors: u64,
}

impl RuntimeStats {
    pub fn new_shared() -> Arc<Self> {
        Arc::new(Self {
            started_at: Instant::now(),
            successful_cycles: AtomicU64::new(0),
            mqtt_messages_sent: AtomicU64::new(0),
            recoveries: AtomicU64::new(0),
            source_bytes_read: AtomicU64::new(0),
            source_bytes_written: AtomicU64::new(0),
            source_frames_read: AtomicU64::new(0),
            source_frames_written: AtomicU64::new(0),
            source_timeouts: AtomicU64::new(0),
            source_errors: AtomicU64::new(0),
        })
    }

    pub fn spawn_reporter(self: &Arc<Self>, interval: Duration) {
        let stats = Arc::clone(self);
        thread::spawn(move || {
            let mut previous = stats.snapshot();
            loop {
                thread::sleep(interval);
                let current = stats.snapshot();
                let delta = current.delta_from(&previous);
                eprintln!("{}", format_summary(&current, &delta));
                previous = current;
            }
        });
    }

    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            uptime: self.started_at.elapsed(),
            successful_cycles: self.successful_cycles.load(Ordering::Relaxed),
            mqtt_messages_sent: self.mqtt_messages_sent.load(Ordering::Relaxed),
            recoveries: self.recoveries.load(Ordering::Relaxed),
            source_bytes_read: self.source_bytes_read.load(Ordering::Relaxed),
            source_bytes_written: self.source_bytes_written.load(Ordering::Relaxed),
            source_frames_read: self.source_frames_read.load(Ordering::Relaxed),
            source_frames_written: self.source_frames_written.load(Ordering::Relaxed),
            source_timeouts: self.source_timeouts.load(Ordering::Relaxed),
            source_errors: self.source_errors.load(Ordering::Relaxed),
        }
    }

    pub fn record_successful_cycle(&self) {
        self.successful_cycles.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_mqtt_message_sent(&self) {
        self.mqtt_messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_recovery(&self) {
        self.recoveries.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_source_write(&self, bytes: usize) {
        self.source_bytes_written
            .fetch_add(bytes as u64, Ordering::Relaxed);
        self.source_frames_written.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_source_read(&self, bytes: usize) {
        self.source_bytes_read
            .fetch_add(bytes as u64, Ordering::Relaxed);
        self.source_frames_read.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_source_timeout(&self) {
        self.source_timeouts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_source_error(&self) {
        self.source_errors.fetch_add(1, Ordering::Relaxed);
    }
}

impl StatsSnapshot {
    pub fn delta_from(&self, previous: &Self) -> StatsDelta {
        StatsDelta {
            successful_cycles: self.successful_cycles - previous.successful_cycles,
            mqtt_messages_sent: self.mqtt_messages_sent - previous.mqtt_messages_sent,
            recoveries: self.recoveries - previous.recoveries,
            source_bytes_read: self.source_bytes_read - previous.source_bytes_read,
            source_bytes_written: self.source_bytes_written - previous.source_bytes_written,
            source_frames_read: self.source_frames_read - previous.source_frames_read,
            source_frames_written: self.source_frames_written - previous.source_frames_written,
            source_timeouts: self.source_timeouts - previous.source_timeouts,
            source_errors: self.source_errors - previous.source_errors,
        }
    }
}

pub fn format_summary(snapshot: &StatsSnapshot, delta: &StatsDelta) -> String {
    let mut summary = String::new();
    let _ = write!(
        summary,
        "alive: uptime={} cycles={} (+{}) mqtt_messages={} (+{}) recoveries={} (+{}) source_rx={} (+{}) source_tx={} (+{}) frames_rx={} (+{}) frames_tx={} (+{}) timeouts={} (+{}) source_errors={} (+{})",
        format_duration(snapshot.uptime),
        snapshot.successful_cycles,
        delta.successful_cycles,
        snapshot.mqtt_messages_sent,
        delta.mqtt_messages_sent,
        snapshot.recoveries,
        delta.recoveries,
        format_bytes(snapshot.source_bytes_read),
        format_bytes(delta.source_bytes_read),
        format_bytes(snapshot.source_bytes_written),
        format_bytes(delta.source_bytes_written),
        snapshot.source_frames_read,
        delta.source_frames_read,
        snapshot.source_frames_written,
        delta.source_frames_written,
        snapshot.source_timeouts,
        delta.source_timeouts,
        snapshot.source_errors,
        delta.source_errors,
    );
    summary
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;

    if bytes < 1024 {
        return format!("{bytes}B");
    }

    let bytes_f64 = bytes as f64;
    if bytes_f64 < MIB {
        return format!("{:.1}KiB", bytes_f64 / KIB);
    }

    format!("{:.1}MiB", bytes_f64 / MIB)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{StatsDelta, StatsSnapshot, format_summary};

    #[test]
    fn format_summary_includes_totals_and_interval_delta() {
        let snapshot = StatsSnapshot {
            uptime: Duration::from_secs(301),
            successful_cycles: 600,
            mqtt_messages_sent: 1200,
            recoveries: 2,
            source_bytes_read: 12_288,
            source_bytes_written: 2_048,
            source_frames_read: 900,
            source_frames_written: 905,
            source_timeouts: 1,
            source_errors: 3,
        };
        let delta = StatsDelta {
            successful_cycles: 300,
            mqtt_messages_sent: 600,
            recoveries: 1,
            source_bytes_read: 4_096,
            source_bytes_written: 1_024,
            source_frames_read: 450,
            source_frames_written: 452,
            source_timeouts: 1,
            source_errors: 2,
        };

        let summary = format_summary(&snapshot, &delta);

        assert!(summary.contains("alive: uptime=00:05:01"));
        assert!(summary.contains("cycles=600 (+300)"));
        assert!(summary.contains("mqtt_messages=1200 (+600)"));
        assert!(summary.contains("recoveries=2 (+1)"));
        assert!(summary.contains("source_rx=12.0KiB (+4.0KiB)"));
        assert!(summary.contains("source_tx=2.0KiB (+1.0KiB)"));
        assert!(summary.contains("timeouts=1 (+1)"));
        assert!(summary.contains("source_errors=3 (+2)"));
    }
}
