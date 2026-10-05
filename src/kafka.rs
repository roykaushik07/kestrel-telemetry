use std::time::Duration;

use kestrel_contracts::TelemetryFrame;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};

use crate::settings::Kafka;

pub struct TelemetryProducer {
    inner: FutureProducer,
    topic: String,
}

impl TelemetryProducer {
    pub fn new(cfg: &Kafka) -> Result<Self, rdkafka::error::KafkaError> {
        let inner: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &cfg.brokers)
            .set("security.protocol", "SSL")
            .set("linger.ms", cfg.linger_ms.to_string())
            .set("compression.type", "lz4")
            .set("enable.idempotence", "true")
            .create()?;
        Ok(Self { inner, topic: cfg.topic.clone() })
    }

    /// Keyed by drone id so a drone's frames stay ordered within a partition.
    pub async fn publish(&self, frame: &TelemetryFrame) -> anyhow::Result<()> {
        let payload = serde_json::to_vec(frame)?;
        let record = FutureRecord::to(&self.topic).key(&frame.drone_id).payload(&payload);
        self.inner
            .send(record, Duration::from_secs(1))
            .await
            .map_err(|(err, _)| anyhow::anyhow!(err))?;
        Ok(())
    }

    pub fn flush(&self) {
        let _ = self.inner.flush(Duration::from_secs(5));
    }
}
