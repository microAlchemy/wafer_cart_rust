use dora_node_api::{DoraNode, Event, IntoArrow, MetadataParameters};
use wafer_cart::{init_logging, runtime::Runtime, unix_ms};
fn main() -> eyre::Result<()> {
    init_logging();
    let (mut node, mut events) = DoraNode::init_from_env()?;
    let mut runtime = Runtime::new()?;
    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, .. } if id.as_str() == "planning_command" => {
                let bytes: &[u8] = (&data).try_into()?;
                let status = runtime.command(bytes, unix_ms());
                node.send_output(
                    "command_status".into(),
                    MetadataParameters::default(),
                    serde_json::to_vec(&status)?.into_arrow(),
                )?;
            }
            Event::Input { id, .. } if id.as_str() == "tick" => {
                let telemetry = serde_json::to_vec(&runtime.telemetry(unix_ms()))?;
                node.send_output(
                    "telemetry".into(),
                    MetadataParameters::default(),
                    telemetry.into_arrow(),
                )?;
            }
            Event::Stop(_) => break,
            Event::Error(error) => eyre::bail!("dora: {error}"),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dora_node_api::integration_testing::{
        self, IntegrationTestInput, TestingInput, TestingOutput,
        integration_testing_format::{IncomingEvent, InputData, TimedIncomingEvent},
    };

    #[test]
    fn dora_command_and_telemetry_round_trip() -> eyre::Result<()> {
        let command = serde_json::to_vec(
            &serde_json::json!({"version":1,"id":"integration-1","issued_at_ms":unix_ms(),"command":{"type":"pause"}}),
        )?;
        let input = |id: &str, bytes: Vec<u8>, time_offset_secs| TimedIncomingEvent {
            time_offset_secs,
            event: IncomingEvent::Input {
                id: id.to_owned().into(),
                metadata: None,
                data: Some(Box::new(InputData::JsonObject {
                    data: serde_json::json!(bytes),
                    data_type: Some(serde_json::json!("UInt8")),
                })),
            },
        };
        let events = vec![
            input("planning_command", command, 0.0),
            input("planning_command", b"bad".to_vec(), 0.01),
            input("tick", vec![], 0.02),
            TimedIncomingEvent {
                time_offset_secs: 0.03,
                event: IncomingEvent::Stop,
            },
        ];
        let (tx, mut rx) = integration_testing::output_channel();
        integration_testing::setup_integration_testing(
            TestingInput::Input(IntegrationTestInput::new("cart".to_owned().into(), events)),
            TestingOutput::ToChannel(tx),
            Default::default(),
        );
        main()?;
        let outputs = integration_testing::drain_outputs(&mut rx);
        assert_eq!(outputs.len(), 3, "{outputs:?}");
        let decode = |output: &integration_testing::OutputJson| -> serde_json::Value {
            let bytes: Vec<u8> = serde_json::from_value(output["data"].clone()).unwrap();
            serde_json::from_slice(&bytes).unwrap()
        };
        assert_eq!(outputs[0]["id"], "command_status");
        assert_eq!(decode(&outputs[0])["accepted"], true);
        assert_eq!(decode(&outputs[1])["accepted"], false);
        assert_eq!(outputs[2]["id"], "telemetry");
        assert_eq!(decode(&outputs[2])["metrics"]["commands_rejected"], 1);
        Ok(())
    }
}
