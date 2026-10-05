use std::time::Duration;

use kestrel_contracts::{Mission, Position, TelemetryFrame};
use moka::future::Cache;

use crate::mavlink::DecodedFrame;
use crate::settings::Dispatch;

/// Resolves the active mission for a drone via dispatch's internal API and
/// builds the contract `TelemetryFrame`. Lookups are cached briefly. A drone
/// with no active mission (or an unreachable dispatch) yields no frame: the
/// contract requires a mission id, so such frames are dropped with a log line.
pub struct Enricher {
    http: reqwest::Client,
    base: String,
    cache: Cache<String, Option<String>>,
}

impl Enricher {
    pub fn new(cfg: &Dispatch) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_millis(cfg.timeout_ms))
            .build()?;
        let cache = Cache::builder().time_to_live(Duration::from_secs(10)).max_capacity(2_000).build();
        Ok(Self { http, base: cfg.url.trim_end_matches('/').to_string(), cache })
    }

    pub async fn enrich(&self, frame: DecodedFrame) -> Option<TelemetryFrame> {
        let drone = frame.drone_id.clone();
        let mission_id = self.cache.get_with(drone.clone(), async { self.lookup(&drone).await }).await;
        let Some(mission_id) = mission_id else {
            tracing::info!(drone_id = %drone, "no active mission, dropping frame");
            return None;
        };
        Some(TelemetryFrame {
            drone_id: frame.drone_id,
            mission_id,
            timestamp: frame.observed_at,
            position: Position {
                latitude: frame.lat,
                longitude: frame.lon,
                altitude_meters: frame.alt_m,
                hdop: None,
            },
            ground_speed_ms: Some(frame.speed_ms),
            heading_degrees: Some(frame.heading_deg),
            battery_percent: frame.battery_percent,
            battery_voltage: frame.battery_voltage,
            payload_temp_celsius: frame.payload_temp_celsius,
            payload_door_closed: None,
            link_quality: None,
        })
    }

    async fn lookup(&self, drone_id: &str) -> Option<String> {
        let url = format!("{}/internal/v1/drones/{}/active-mission", self.base, drone_id);
        let resp = match self.http.get(url).send().await {
            Ok(r) => r,
            Err(err) => {
                tracing::debug!(%err, drone_id, "dispatch lookup failed");
                return None;
            }
        };
        if !resp.status().is_success() {
            return None;
        }
        resp.json::<Mission>().await.ok().map(|m| m.id)
    }
}
