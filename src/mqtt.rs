use std::time::Duration;

use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS, Transport};
use tokio::sync::mpsc::Sender;
use tracing::{error, info};

use crate::settings::Mqtt;

/// A raw MAVLink payload as received from a drone, before decoding.
#[derive(Debug, Clone)]
pub struct RawFrame {
    pub drone_id: String,
    pub payload: Vec<u8>,
}

/// Subscribes to `drones/+/mavlink` and forwards payloads to the pipeline.
pub async fn run(cfg: Mqtt, tx: Sender<RawFrame>) {
    let url = format!("{}?client_id={}", cfg.broker, cfg.client_id);
    let mut opts = match MqttOptions::parse_url(url) {
        Ok(o) => o,
        Err(err) => {
            error!(%err, "invalid mqtt broker url");
            return;
        }
    };
    opts.set_keep_alive(Duration::from_secs(15));
    opts.set_transport(Transport::tls_with_default_config());

    let (client, mut eventloop) = AsyncClient::new(opts, 256);
    if let Err(err) = client.subscribe("drones/+/mavlink", QoS::AtLeastOnce).await {
        error!(%err, "subscribe failed");
        return;
    }

    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::Publish(p))) => {
                if let Some(drone_id) = drone_from_topic(&p.topic) {
                    let frame = RawFrame { drone_id, payload: p.payload.to_vec() };
                    if tx.send(frame).await.is_err() {
                        return;
                    }
                }
            }
            Ok(Event::Incoming(Packet::ConnAck(_))) => info!("mqtt connected"),
            Ok(_) => {}
            Err(err) => {
                error!(%err, "mqtt connection error, retrying");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

fn drone_from_topic(topic: &str) -> Option<String> {
    let mut parts = topic.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some("drones"), Some(id), Some("mavlink")) => Some(id.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_drone_id() {
        assert_eq!(drone_from_topic("drones/kd-0042/mavlink").as_deref(), Some("kd-0042"));
        assert_eq!(drone_from_topic("drones/kd-0042/status"), None);
    }
}
