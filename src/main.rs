use wafer_cart::{
    init_logging,
    runtime::{Runtime, diagnostics},
    unix_ms,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();
    match std::env::args().nth(1).as_deref() {
        Some("diagnostics") => println!("{}", serde_json::to_string(&diagnostics()?)?),
        Some("demo") => {
            let mut runtime = Runtime::new()?;
            let now = unix_ms();
            let command = serde_json::json!({"version":1,"id":"demo-1","issued_at_ms":now,"command":{"type":"goal","waypoints":[{"x_m":1.0,"y_m":0.0,"heading_rad":0.0}]}});
            println!(
                "{}",
                serde_json::to_string(&runtime.command(&serde_json::to_vec(&command)?, now))?
            );
            println!("{}", serde_json::to_string(&runtime.telemetry(now))?);
        }
        _ => return Err("usage: cargo run --bin wafer-cart -- <diagnostics|demo>".into()),
    }
    Ok(())
}
