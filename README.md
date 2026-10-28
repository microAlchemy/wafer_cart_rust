# Wafer Cart — Rust foundation

A simulation-first Rust library and dora-rs telemetry node alongside the original Python controller. Python files are unchanged. No physical device is opened or moved.

## Goal status

| Goal | What is implemented | Still needed |
| --- | --- | --- |
| Dora telemetry node for the server | `telemetry-node` publishes a telemetry snapshot every 100 ms and command acknowledgements through Dora. `server-demo` exercises the dataflow locally. | Connect the real server and run a live Dora dataflow. |
| Read upstream global planning commands | Versioned JSON goal, pause, resume, and stop commands are validated and acknowledged. Goals update planning state without moving motors. | Define the real server transport and authentication; add route execution only when intended. |
| Low level camera, motor, and IR library | Rust traits, validated data types, deterministic simulators, motor bounds, and a simulated watchdog and emergency stop. | Implement and calibrate physical device adapters and an independent hardware watchdog. |
| Metrics, reporting, logging, diagnostics, tests | Telemetry includes counters, latency, and device health; JSON logs use `RUST_LOG`; simulated diagnostics and tests cover the protocol and Dora message exchange. | Verify on real hardware and in a live server deployment. |

This completes the **Rust foundation** for the four goals, not production integration with a server or physical cart.

## Run locally

```sh
cargo run --bin wafer-cart -- diagnostics
cargo run --bin wafer-cart -- demo
cargo test --workspace --all-features
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo fmt --check
```

`diagnostics` validates simulated camera/IR reads and motor stop behavior, prints JSON, and exits nonzero on failure. `demo` submits a planning goal and prints its acknowledgement plus a telemetry snapshot. Structured JSON logs go to stderr; set `RUST_LOG=debug` to configure filtering.

## Dora dataflow

The optional `dora` feature uses the real `dora-node-api` 1.1.0. With a compatible dora CLI installed:

```sh
cargo build --features dora --bins
dora run examples/dataflow.yml
```

The `server-demo` node sends a goal every second and logs acknowledgements and telemetry received from `cart`. Cart publishes telemetry every 100 ms. Stop the flow with Ctrl-C. The CLI/daemon is not bundled. The test suite exercises the node through dora's integration-testing API without a daemon.

| Port | Direction | Payload |
| --- | --- | --- |
| `planning_command` | Input | UTF-8 JSON in an Arrow UInt8 array |
| `tick` | Input | Timer event; contents ignored |
| `command_status` | Output | JSON: ID (null if invalid), accepted, message |
| `telemetry` | Output | JSON: timestamp, uptime, planning state, IR, camera health, motor state, metrics |

Replace `server-demo` with your server-side dora node or a trusted gateway that emits `planning_command` and consumes the two outputs. This slice defines the dora boundary; it does **not** implement HTTP/WebSocket transport, server authentication, remote deployment, navigation, or motion execution. Keep the boundary on a trusted dataflow until the server's transport/authentication contract is defined. See the [official Rust API](https://github.com/dora-rs/dora/blob/main/docs/api-rust.md).

## Planning protocol v1

```json
{
  "version": 1,
  "id": "mission-42",
  "issued_at_ms": 1792411200000,
  "command": {
    "type": "goal",
    "waypoints": [{"x_m": 1.0, "y_m": 0.0, "heading_rad": 0.0}]
  }
}
```

Set `issued_at_ms` to the current Unix time in milliseconds (the static example expires). Sender and receiver clocks must be synchronized: commands older than 30 seconds or in the future are rejected. Positions use the caller's agreed local coordinate frame in meters; headings use radians. Limits: 64 KiB command, 128-byte nonempty ID, 1–256 waypoints, finite coordinates within ±100 km and headings within ±π. Unknown fields/types and invalid JSON are rejected. The most recent 1,024 accepted IDs are retained for duplicate rejection, not durable replay protection.

Other command bodies: `{"type":"pause"}`, `{"type":"resume"}`, `{"type":"stop"}`. Goals are stored and reported; an accepted acknowledgement means planning state was updated, not that a route was executed. Start paused and disarmed. Resume only updates planning state. Stop clears the goal and latches emergency stop; subsequent non-stop commands are rejected until a local runtime restart. No server command arms motors.

## Hardware library and diagnostics

- `Camera`: fallible RGB8 frame capture with dimension/size validation (16 MiB maximum).
- `IrSensor`: four normalized, finite readings in `[0, 1]`.
- `MotorController`: explicit arm, bounded differential drive demands in `[-1, 1]`, disarm/stop, latched emergency stop.
- `SimCamera`, `SimIr`, `SimMotor`: deterministic adapters usable without hardware. Invalid drive demands disarm the motor. The simulator checks its monotonic watchdog during drive/state/tick processing and stops on drop.

Physical adapters still need board/bus details, camera backend, motor channel/polarity/calibration, IR calibration, process ownership, and an **independent hardware watchdog**. The simulator's cooperative watchdog and Rust drop cleanup cannot protect a physical actuator against process hangs, power loss, or forced termination. The runtime currently constructs simulators explicitly; wire physical implementations only after those contracts are defined.

Telemetry reports accepted/rejected commands, processing latency, telemetry sample count, hardware failures and watchdog stops. Camera health captures/validates a frame; frame pixels are not streamed. Diagnostics and tests cover validation, duplicate/expired commands, goal state, stop latch, watchdog, and dora command/telemetry exchange. Actual hardware and a network server are not tested.
