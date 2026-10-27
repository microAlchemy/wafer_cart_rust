use serde::{Deserialize, Serialize};

pub const MAX_COMMAND_BYTES: usize = 64 * 1024;
pub const MAX_AGE_MS: u64 = 30_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandEnvelope {
    pub version: u8,
    pub id: String,
    pub issued_at_ms: u64,
    pub command: PlanningCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanningCommand {
    Goal { waypoints: Vec<Waypoint> },
    Pause {},
    Resume {},
    Stop {},
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Waypoint {
    pub x_m: f64,
    pub y_m: f64,
    pub heading_rad: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("command exceeds 64 KiB")]
    TooLarge,
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported version or invalid command ID")]
    Envelope,
    #[error("command expired or timestamp is in the future")]
    Timestamp,
    #[error("expected 1..=256 finite waypoints within 100 km and heading +/- pi")]
    Waypoints,
}

impl CommandEnvelope {
    pub fn parse(bytes: &[u8], now_ms: u64) -> Result<Self, CommandError> {
        if bytes.len() > MAX_COMMAND_BYTES {
            return Err(CommandError::TooLarge);
        }
        let value: Self = serde_json::from_slice(bytes)?;
        if value.version != 1
            || value.id.is_empty()
            || value.id.len() > 128
            || value.id.chars().any(char::is_control)
        {
            return Err(CommandError::Envelope);
        }
        if value.issued_at_ms > now_ms || now_ms - value.issued_at_ms > MAX_AGE_MS {
            return Err(CommandError::Timestamp);
        }
        if let PlanningCommand::Goal { waypoints } = &value.command
            && (waypoints.is_empty()
                || waypoints.len() > 256
                || waypoints.iter().any(|p| {
                    !p.x_m.is_finite()
                        || !p.y_m.is_finite()
                        || !p.heading_rad.is_finite()
                        || p.x_m.abs() > 100_000.0
                        || p.y_m.abs() > 100_000.0
                        || p.heading_rad.abs() > std::f64::consts::PI
                }))
        {
            return Err(CommandError::Waypoints);
        }
        Ok(value)
    }
}
