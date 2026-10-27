# Rust foundation

Objective: a runnable Rust foundation alongside the existing Python prototype: dora telemetry and upstream planning commands, reusable camera/motor/IR interfaces, metrics, logs and diagnostics.
Baseline: c82faadea67a1eab1873f913c527b53e9edc2d20; hardware.py and main.py have user modifications. All Python files must remain untouched. No Rust project or server protocol currently exists.

One implementation phase, implemented directly by Codex under the user override: “Dont use the deepseek worker, just use regular codex”. Establish typed protocol and hardware contracts, implement simulator and runtime, then dora integration and diagnostics/tests/docs. Astra owns review and this plan.

Architecture: small Cargo workspace or single package with cohesive modules. serde versioned JSON command envelopes with command ID and timestamp; typed planning commands (goal/waypoints, pause/resume, stop) are received and acknowledged, not translated into motor movement. Unknown/malformed/expired/non-finite commands rejected. dora input `planning_command`, timer `tick`; outputs `telemetry` and `command_status`, encoded as UTF-8 JSON bytes using the real dora Rust API. Transport to remote server is outside this slice: provide explicit dora wiring and a local command producer/demo. No invented production auth or network protocol.

Hardware library: fallible Camera, MotorController and IrSensor traits, typed frames/readings, deterministic simulators, validated bounded motor commands, explicit arm/disarm and latched emergency stop. Start disarmed; stop on runtime exit/error with guard semantics. No physical bus writes or speculative board drivers. Describe physical adapter requirements.

Diagnostics: tracing logs configurable through RUST_LOG, uptime/counters/latency and hardware health snapshots; CLI self-test runs on simulation and returns meaningful exit status. Bounded payload/frame/waypoint limits; watchdog using monotonic elapsed time for any armed simulated actuator; no autonomous motor execution from goals.

Acceptance: cargo fmt --check, cargo test --workspace, cargo clippy --workspace --all-targets -- -D warnings; compile dora integration (feature-gated if appropriate), run simulator diagnostics and command/telemetry smoke path. Tests cover malformed/stale commands, limits, emergency-stop latch, motor bounds, watchdog, and diagnostics. README gives exact quickstart, protocol examples, server boundary and hardware limitations. Verify current dora API from primary docs/source. Do not claim live dora or hardware tests unless run.

Scope: new Rust manifests/lockfile/src/crates/tests/examples/configs, README.md, .gitignore and unique worker report. No changes to Python or global configuration; no commits/push/deploy. Root plan/checkpoint reserved for Astra.
