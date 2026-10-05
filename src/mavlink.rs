use std::collections::HashMap;

use chrono::{DateTime, Utc};
use mavlink::common::MavMessage;
use mavlink::Message;
use thiserror::Error;

use crate::mqtt::RawFrame;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("mavlink parse error: {0}")]
    Parse(String),
    #[error("unsupported message id {0}")]
    Unsupported(u32),
}

/// One decoded MAVLink message we care about.
#[derive(Debug, Clone, PartialEq)]
pub enum Update {
    Position { lat: f64, lon: f64, alt_m: f64, speed_ms: f64, heading_deg: f64 },
    Battery { percent: f64, voltage: Option<f64> },
    PayloadTemp { celsius: f64 },
}

/// A full snapshot of a drone, assembled from several messages. It does not
/// know its mission yet: the contract `TelemetryFrame` requires a mission id,
/// which only dispatch can supply (see `enrich`).
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    pub drone_id: String,
    pub observed_at: DateTime<Utc>,
    pub lat: f64,
    pub lon: f64,
    pub alt_m: f64,
    pub speed_ms: f64,
    pub heading_deg: f64,
    pub battery_percent: f64,
    pub battery_voltage: Option<f64>,
    pub payload_temp_celsius: f64,
}

/// Decodes a MAVLink v2 payload. Payload temperature is carried by the drone's
/// companion computer as a NAMED_VALUE_FLOAT called `PLD_TEMP`.
pub fn decode(raw: &RawFrame) -> Result<Update, DecodeError> {
    let mut reader = mavlink::peek_reader::PeekReader::new(raw.payload.as_slice());
    let (_header, msg) = mavlink::read_v2_msg::<MavMessage, _>(&mut reader)
        .map_err(|e| DecodeError::Parse(e.to_string()))?;

    match msg {
        MavMessage::GLOBAL_POSITION_INT(p) => Ok(Update::Position {
            lat: p.lat as f64 / 1e7,
            lon: p.lon as f64 / 1e7,
            alt_m: p.alt as f64 / 1000.0,
            speed_ms: (p.vx as f64).hypot(p.vy as f64) / 100.0,
            heading_deg: p.hdg as f64 / 100.0,
        }),
        MavMessage::SYS_STATUS(s) => Ok(Update::Battery {
            percent: s.battery_remaining.max(0) as f64,
            voltage: (s.voltage_battery != u16::MAX).then(|| s.voltage_battery as f64 / 1000.0),
        }),
        MavMessage::NAMED_VALUE_FLOAT(v) if name_of(&v.name) == "PLD_TEMP" => {
            Ok(Update::PayloadTemp { celsius: v.value as f64 })
        }
        other => Err(DecodeError::Unsupported(other.message_id())),
    }
}

fn name_of(raw: &[u8]) -> String {
    let end = raw.iter().position(|b| *b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

#[derive(Default)]
struct DroneState {
    battery: Option<(f64, Option<f64>)>,
    temp: Option<f64>,
}

/// Merges per-message updates into complete frames. A frame is emitted on each
/// position update once battery and payload temperature have been seen.
#[derive(Default)]
pub struct Assembler {
    drones: HashMap<String, DroneState>,
}

impl Assembler {
    pub fn apply(&mut self, drone_id: &str, update: Update, now: DateTime<Utc>) -> Option<DecodedFrame> {
        let state = self.drones.entry(drone_id.to_string()).or_default();
        match update {
            Update::Battery { percent, voltage } => {
                state.battery = Some((percent, voltage));
                None
            }
            Update::PayloadTemp { celsius } => {
                state.temp = Some(celsius);
                None
            }
            Update::Position { lat, lon, alt_m, speed_ms, heading_deg } => {
                let (battery_percent, battery_voltage) = state.battery?;
                Some(DecodedFrame {
                    drone_id: drone_id.to_string(),
                    observed_at: now,
                    lat,
                    lon,
                    alt_m,
                    speed_ms,
                    heading_deg,
                    battery_percent,
                    battery_voltage,
                    payload_temp_celsius: state.temp?,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_only_when_complete() {
        let mut a = Assembler::default();
        let now = Utc::now();
        let pos = Update::Position { lat: 51.5, lon: -0.1, alt_m: 90.0, speed_ms: 18.0, heading_deg: 270.0 };
        assert!(a.apply("kd-1", pos.clone(), now).is_none());
        a.apply("kd-1", Update::Battery { percent: 80.0, voltage: Some(22.4) }, now);
        a.apply("kd-1", Update::PayloadTemp { celsius: 4.2 }, now);
        let frame = a.apply("kd-1", pos, now).expect("complete frame");
        assert_eq!(frame.payload_temp_celsius, 4.2);
    }
}
