use dora_node_api::{DoraNode, Event, IntoArrow, MetadataParameters};
use wafer_cart::{init_logging, unix_ms};
fn main() -> eyre::Result<()> {
    init_logging();
    let (mut node, mut events) = DoraNode::init_from_env()?;
    let mut sequence = 0_u64;
    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, .. } if id.as_str() == "tick" => {
                sequence += 1;
                let command = serde_json::json!({"version":1,"id":format!("demo-{sequence}"),"issued_at_ms":unix_ms(),"command":{"type":"goal","waypoints":[{"x_m":1.0,"y_m":0.0,"heading_rad":0.0}]}});
                node.send_output(
                    "planning_command".into(),
                    MetadataParameters::default(),
                    serde_json::to_vec(&command)?.into_arrow(),
                )?;
            }
            Event::Input { id, data, .. } => {
                let bytes: &[u8] = (&data).try_into()?;
                tracing::info!(input = %id, payload = %std::str::from_utf8(bytes)?, "server received");
            }
            Event::Stop(_) => break,
            Event::Error(error) => eyre::bail!("dora: {error}"),
            _ => {}
        }
    }
    Ok(())
}
