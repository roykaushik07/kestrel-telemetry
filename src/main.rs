mod api;
mod enrich;
mod kafka;
mod mavlink;
mod mqtt;
mod settings;

use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let settings = settings::Settings::load()?;
    info!(broker = %settings.mqtt.broker, topic = %settings.kafka.topic, "starting kestrel-telemetry");

    let window = Arc::new(api::HotWindow::new(settings.api.window_secs));
    let enricher = enrich::Enricher::new(&settings.dispatch)?;
    let producer = kafka::TelemetryProducer::new(&settings.kafka)?;

    let mut assembler = mavlink::Assembler::default();

    let (raw_tx, mut raw_rx) = mpsc::channel::<mqtt::RawFrame>(4096);
    tokio::spawn(mqtt::run(settings.mqtt.clone(), raw_tx));

    let api_window = window.clone();
    let bind = settings.api.bind.clone();
    tokio::spawn(async move {
        if let Err(err) = api::serve(&bind, api_window).await {
            tracing::error!(%err, "query api exited");
        }
    });

    loop {
        tokio::select! {
            Some(raw) = raw_rx.recv() => {
                let update = match mavlink::decode(&raw) {
                    Ok(u) => u,
                    Err(err) => { tracing::debug!(%err, drone = %raw.drone_id, "skipping message"); continue; }
                };
                let Some(decoded) = assembler.apply(&raw.drone_id, update, chrono::Utc::now()) else { continue };
                let Some(frame) = enricher.enrich(decoded).await else { continue };
                window.push(frame.clone());
                if let Err(err) = producer.publish(&frame).await {
                    tracing::error!(%err, "kafka publish failed");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("shutting down");
                producer.flush();
                return Ok(());
            }
        }
    }
}
