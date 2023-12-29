// use cluster::rpc::connector::MediaEndpointLogRequest;

// use super::ConnectorEventSerializer;

// pub struct JsonConnectorEventSerializer;

// impl ConnectorEventSerializer for JsonConnectorEventSerializer {
//     fn serialize(&self, event: &MediaEndpointLogRequest) -> Result<Vec<u8>, String> {
//         let data = serde_json::to_vec(event).map_err(|e| e.to_string())?;
//         Ok(data)
//     }
// }
