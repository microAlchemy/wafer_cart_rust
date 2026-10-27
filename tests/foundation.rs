use std::time::{Duration, Instant};
use wafer_cart::{hardware::*, protocol::*, runtime::*};
fn command(id: &str, kind: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"version":1,"id":id,"issued_at_ms":1000,"command":kind}))
        .unwrap()
}
#[test]
fn validates_commands() {
    let valid = command("a", serde_json::json!({"type":"pause"}));
    assert!(CommandEnvelope::parse(&valid, 1000).is_ok());
    assert!(CommandEnvelope::parse(&valid, 999).is_err());
    assert!(CommandEnvelope::parse(&valid, 31_001).is_err());
    assert!(CommandEnvelope::parse(b"{", 1000).is_err());
    assert!(CommandEnvelope::parse(&vec![0; MAX_COMMAND_BYTES + 1], 1000).is_err());
    for kind in [
        serde_json::json!({"type":"unknown"}),
        serde_json::json!({"type":"goal","waypoints":[]}),
        serde_json::json!({"type":"pause","extra":true}),
        serde_json::json!({"type":"goal","waypoints":[{"x_m":100001,"y_m":0,"heading_rad":0}]}),
    ] {
        assert!(CommandEnvelope::parse(&command("a", kind), 1000).is_err());
    }
    assert!(
        CommandEnvelope::parse(&command("", serde_json::json!({"type":"pause"})), 1000).is_err()
    );
}
#[test]
fn motor_is_disarmed_bounded_and_latched() {
    let mut motor = SimMotor::new(Duration::from_secs(1)).unwrap();
    let demand = MotorDemand {
        left: 0.2,
        right: -0.2,
    };
    assert!(motor.drive(demand).is_err());
    motor.arm().unwrap();
    motor.drive(demand).unwrap();
    assert_eq!(motor.state().demand, demand);
    assert!(
        motor
            .drive(MotorDemand {
                left: f32::NAN,
                right: 0.0
            })
            .is_err()
    );
    assert!(!motor.state().armed);
    assert_eq!(motor.state().demand, MotorDemand::default());
    motor.arm().unwrap();
    assert!(
        motor
            .drive(MotorDemand {
                left: 1.1,
                right: 0.0
            })
            .is_err()
    );
    motor.emergency_stop().unwrap();
    assert!(motor.arm().is_err());
    assert!(motor.drive(demand).is_err());
}
#[test]
fn watchdog_disarms() {
    let mut motor = SimMotor::new(Duration::from_millis(10)).unwrap();
    motor.arm().unwrap();
    motor
        .drive(MotorDemand {
            left: 0.5,
            right: 0.5,
        })
        .unwrap();
    assert!(motor.watchdog(Instant::now() + Duration::from_millis(11)));
    assert!(!motor.state().armed);
    assert_eq!(motor.state().demand, MotorDemand::default());
    assert!(!motor.watchdog(Instant::now() + Duration::from_secs(1)));
}
#[test]
fn runtime_tracks_goals_and_rejections_without_motion() {
    let mut runtime = Runtime::new().unwrap();
    let goal = command(
        "goal-1",
        serde_json::json!({"type":"goal","waypoints":[{"x_m":1,"y_m":2,"heading_rad":0}]}),
    );
    assert!(runtime.command(&goal, 1000).accepted);
    assert!(!runtime.command(&goal, 1000).accepted);
    assert!(!runtime.command(b"bad", 1000).accepted);
    let telemetry = runtime.telemetry(1000);
    assert!(telemetry.goal.is_some());
    assert!(!telemetry.motor.armed);
    assert_eq!(telemetry.metrics.commands_accepted, 1);
    assert_eq!(telemetry.metrics.commands_rejected, 2);
    assert!(telemetry.camera_healthy);
    assert!(
        runtime
            .command(&command("stop", serde_json::json!({"type":"stop"})), 1000)
            .accepted
    );
    assert!(
        !runtime
            .command(
                &command("resume", serde_json::json!({"type":"resume"})),
                1000
            )
            .accepted
    );
    let telemetry = runtime.telemetry(1000);
    assert!(telemetry.goal.is_none());
    assert!(telemetry.motor.emergency_latched);
}
#[test]
fn hardware_validation_and_diagnostics() {
    assert!(
        Frame {
            width: u32::MAX,
            height: u32::MAX,
            rgb: vec![]
        }
        .validate()
        .is_err()
    );
    assert!(
        Frame {
            width: 2,
            height: 2,
            rgb: vec![0; 11]
        }
        .validate()
        .is_err()
    );
    assert!(
        IrReading {
            normalized: [0.0, 1.0, 0.5, f32::INFINITY]
        }
        .validate()
        .is_err()
    );
    assert!(SimMotor::new(Duration::ZERO).is_err());
    let report = diagnostics().unwrap();
    assert!(report.camera_ok && report.ir_ok && report.motor_ok);
}
