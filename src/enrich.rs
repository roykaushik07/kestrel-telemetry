use std::time::Duration;

use kestrel_contracts::{Mission, TelemetryFrame};
use moka::future::Cache;

use crate::settings::Dispatch;

/// Resolves the active mission for a drone via dispatch's internal API and
/// stamps it on the frame. Lookups are cached briefly; on timeout the frame
/// goes out un-enriched rather than stalling the pipeline.
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

    pub async fn enrich(&self, mut frame: TelemetryFrame) -> TelemetryFrame {
        let drone = frame.drone_id.clone();
        frame.mission_id = self
            .cache
            .get_with(drone.clone(), async { self.lookup(&drone).await })
            .await;
        frame
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
