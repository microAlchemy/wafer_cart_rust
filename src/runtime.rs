use crate::{hardware::*, protocol::*};
use serde::Serialize;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

#[derive(Debug, Serialize)]
pub struct CommandStatus {
    pub id: Option<String>,
    pub accepted: bool,
    pub message: String,
}
#[derive(Debug, Default, Serialize)]
pub struct Metrics {
    pub commands_accepted: u64,
    pub commands_rejected: u64,
    pub telemetry_samples: u64,
    pub hardware_errors: u64,
    pub watchdog_stops: u64,
    pub last_command_latency_us: u64,
}
#[derive(Debug, Serialize)]
pub struct Telemetry<'a> {
    pub version: u8,
    pub timestamp_ms: u64,
    pub uptime_ms: u64,
    pub simulated: bool,
    pub paused: bool,
    pub goal: &'a Option<PlanningCommand>,
    pub ir: Option<IrReading>,
    pub camera_healthy: bool,
    pub motor: &'a MotorState,
    pub metrics: &'a Metrics,
}
pub struct Runtime {
    started: Instant,
    camera: SimCamera,
    ir: SimIr,
    motor: SimMotor,
    pub metrics: Metrics,
    goal: Option<PlanningCommand>,
    paused: bool,
    recent: VecDeque<String>,
}
impl Runtime {
    pub fn new() -> Result<Self> {
        Ok(Self {
            started: Instant::now(),
            camera: SimCamera,
            ir: SimIr,
            motor: SimMotor::new(Duration::from_millis(500))?,
            metrics: Metrics::default(),
            goal: None,
            paused: true,
            recent: VecDeque::new(),
        })
    }
    pub fn command(&mut self, bytes: &[u8], now_ms: u64) -> CommandStatus {
        let start = Instant::now();
        let status = match CommandEnvelope::parse(bytes, now_ms) {
            Err(error) => CommandStatus {
                id: None,
                accepted: false,
                message: error.to_string(),
            },
            Ok(command) => {
                let id = Some(command.id.clone());
                if self.recent.contains(&command.id) {
                    CommandStatus {
                        id,
                        accepted: false,
                        message: "duplicate command ID".into(),
                    }
                } else if self.motor.state().emergency_latched
                    && !matches!(command.command, PlanningCommand::Stop { .. })
                {
                    CommandStatus {
                        id,
                        accepted: false,
                        message: "stop latched; local restart required".into(),
                    }
                } else {
                    match command.command {
                        goal @ PlanningCommand::Goal { .. } => self.goal = Some(goal),
                        PlanningCommand::Pause {} => {
                            self.paused = true;
                            let _ = self.motor.stop();
                        }
                        PlanningCommand::Resume {} => self.paused = false,
                        PlanningCommand::Stop {} => {
                            self.paused = true;
                            self.goal = None;
                            let _ = self.motor.emergency_stop();
                        }
                    }
                    if self.recent.len() == 1024 {
                        self.recent.pop_front();
                    }
                    self.recent.push_back(command.id);
                    CommandStatus {
                        id,
                        accepted: true,
                        message: "planning state updated; no actuator execution".into(),
                    }
                }
            }
        };
        if status.accepted {
            self.metrics.commands_accepted += 1;
        } else {
            self.metrics.commands_rejected += 1;
        }
        self.metrics.last_command_latency_us =
            start.elapsed().as_micros().min(u64::MAX as u128) as u64;
        tracing::info!(accepted = status.accepted, command_id = ?status.id, message = %status.message, "planning command");
        status
    }
    pub fn telemetry(&mut self, now_ms: u64) -> Telemetry<'_> {
        if self.motor.watchdog(Instant::now()) {
            self.metrics.watchdog_stops += 1;
        }
        let ir = self
            .ir
            .read()
            .and_then(|r| {
                r.validate()?;
                Ok(r)
            })
            .ok();
        let camera_healthy = self.camera.capture().and_then(|f| f.validate()).is_ok();
        self.metrics.hardware_errors += u64::from(ir.is_none()) + u64::from(!camera_healthy);
        self.metrics.telemetry_samples += 1;
        Telemetry {
            version: 1,
            timestamp_ms: now_ms,
            uptime_ms: self.started.elapsed().as_millis().min(u64::MAX as u128) as u64,
            simulated: true,
            paused: self.paused,
            goal: &self.goal,
            ir,
            camera_healthy,
            motor: self.motor.state(),
            metrics: &self.metrics,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DiagnosticReport {
    pub simulated: bool,
    pub camera_ok: bool,
    pub ir_ok: bool,
    pub motor_ok: bool,
}
pub fn diagnostics() -> Result<DiagnosticReport> {
    SimCamera.capture()?.validate()?;
    SimIr.read()?.validate()?;
    let mut motor = SimMotor::new(Duration::from_millis(500))?;
    motor.arm()?;
    motor.drive(MotorDemand {
        left: 0.1,
        right: -0.1,
    })?;
    motor.emergency_stop()?;
    if motor.arm().is_ok() || motor.state().demand != MotorDemand::default() {
        return Err(HardwareError::Invalid("emergency stop diagnostic"));
    }
    Ok(DiagnosticReport {
        simulated: true,
        camera_ok: true,
        ir_ok: true,
        motor_ok: true,
    })
}
