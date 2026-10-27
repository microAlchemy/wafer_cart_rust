use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Debug, thiserror::Error)]
pub enum HardwareError {
    #[error("invalid hardware value: {0}")]
    Invalid(&'static str),
    #[error("motor controller is disarmed or emergency stop is latched")]
    Disarmed,
    #[error("hardware I/O: {0}")]
    Io(#[from] std::io::Error),
}
pub type Result<T> = std::result::Result<T, HardwareError>;

/// Packed RGB8, row-major, with no stride padding.
#[derive(Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}
impl Frame {
    pub fn validate(&self) -> Result<()> {
        let size = u64::from(self.width)
            .checked_mul(u64::from(self.height))
            .and_then(|pixels| pixels.checked_mul(3))
            .ok_or(HardwareError::Invalid("frame size overflow"))?;
        if self.width == 0
            || self.height == 0
            || size > 16 * 1024 * 1024
            || size != self.rgb.len() as u64
        {
            return Err(HardwareError::Invalid("RGB frame dimensions/size"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct IrReading {
    pub normalized: [f32; 4],
}
impl IrReading {
    pub fn validate(&self) -> Result<()> {
        if self
            .normalized
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err(HardwareError::Invalid("IR range"));
        }
        Ok(())
    }
}
/// Differential left/right demand in [-1, 1]; physical adapters define calibration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct MotorDemand {
    pub left: f32,
    pub right: f32,
}
impl MotorDemand {
    pub fn validate(self) -> Result<()> {
        if [self.left, self.right]
            .iter()
            .any(|v| !v.is_finite() || !(-1.0..=1.0).contains(v))
        {
            return Err(HardwareError::Invalid("motor demand range"));
        }
        Ok(())
    }
}
pub trait Camera {
    fn capture(&mut self) -> Result<Frame>;
}
pub trait IrSensor {
    fn read(&mut self) -> Result<IrReading>;
}
/// Physical implementations must enforce an independent watchdog and stop on drop.
pub trait MotorController {
    fn arm(&mut self) -> Result<()>;
    fn drive(&mut self, demand: MotorDemand) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn emergency_stop(&mut self) -> Result<()>;
}
#[derive(Default)]
pub struct SimCamera;
impl Camera for SimCamera {
    fn capture(&mut self) -> Result<Frame> {
        Ok(Frame {
            width: 8,
            height: 8,
            rgb: vec![64; 8 * 8 * 3],
        })
    }
}
#[derive(Default)]
pub struct SimIr;
impl IrSensor for SimIr {
    fn read(&mut self) -> Result<IrReading> {
        Ok(IrReading {
            normalized: [1.0, 1.0, 0.0, 0.0],
        })
    }
}
#[derive(Debug, Serialize)]
pub struct MotorState {
    pub armed: bool,
    pub emergency_latched: bool,
    pub demand: MotorDemand,
}
pub struct SimMotor {
    state: MotorState,
    last_feed: Instant,
    timeout: Duration,
}
impl SimMotor {
    pub fn new(timeout: Duration) -> Result<Self> {
        if timeout.is_zero() {
            return Err(HardwareError::Invalid("zero watchdog timeout"));
        }
        Ok(Self {
            state: MotorState {
                armed: false,
                emergency_latched: false,
                demand: MotorDemand::default(),
            },
            last_feed: Instant::now(),
            timeout,
        })
    }
    /// Call on every event/timer. Simulation only; physical drivers need a hardware watchdog.
    pub fn watchdog(&mut self, now: Instant) -> bool {
        if self.state.armed && now.saturating_duration_since(self.last_feed) >= self.timeout {
            let _ = self.stop();
            return true;
        }
        false
    }
    pub fn state(&mut self) -> &MotorState {
        self.watchdog(Instant::now());
        &self.state
    }
}
impl MotorController for SimMotor {
    fn arm(&mut self) -> Result<()> {
        if self.state.emergency_latched {
            return Err(HardwareError::Disarmed);
        }
        self.state.armed = true;
        self.last_feed = Instant::now();
        Ok(())
    }
    fn drive(&mut self, demand: MotorDemand) -> Result<()> {
        self.watchdog(Instant::now());
        if let Err(error) = demand.validate() {
            self.stop()?;
            return Err(error);
        }
        if !self.state.armed || self.state.emergency_latched {
            return Err(HardwareError::Disarmed);
        }
        self.state.demand = demand;
        self.last_feed = Instant::now();
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        self.state.armed = false;
        self.state.demand = MotorDemand::default();
        Ok(())
    }
    fn emergency_stop(&mut self) -> Result<()> {
        self.state.emergency_latched = true;
        self.stop()
    }
}
impl Drop for SimMotor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
