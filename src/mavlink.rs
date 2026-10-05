use kestrel_contracts::TelemetryFrame;
use mavlink::common::MavMessage;
use mavlink::{MavConnection, Message};
use thiserror::Error;

use crate::mqtt::RawFrame;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("mavlink parse error: {0}")]
    Parse(String),
    #[error("unsupported message id {0}")]
    Unsupported(u32),
}

/// Decodes a MAVLink v2 payload into a TelemetryFrame. Only GLOBAL_POSITION_INT
/// is consumed; battery and payload temperature arrive on separate messages and
/// are merged downstream by the contracts crate.
pub fn decode(raw: &RawFrame) -> Result<TelemetryFrame, DecodeError> {
    let mut reader = mavlink::peek_reader::PeekReader::new(raw.payload.as_slice());
    let (_header, msg) = mavlink::read_v2_msg::<MavMessage, _>(&mut reader)
        .map_err(|e| DecodeError::Parse(e.to_string()))?;

    match msg {
        MavMessage::GLOBAL_POSITION_INT(p) => Ok(TelemetryFrame {
            drone_id: raw.drone_id.clone(),
            mission_id: None,
            lat: p.lat as f64 / 1e7,
            lon: p.lon as f64 / 1e7,
            alt_m: p.alt as f32 / 1000.0,
            ground_speed_ms: ((p.vx as f32).hypot(p.vy as f32)) / 100.0,
            heading_deg: (p.hdg as f32) / 100.0,
            observed_at_ms: p.time_boot_ms as u64,
        }),
        other => Err(DecodeError::Unsupported(other.message_id())),
    }
}
