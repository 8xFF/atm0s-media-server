use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct MsidInfo {
    pub label: String,
    pub kind: String,
    pub name: String,
}

#[derive(Default)]
pub struct MsidAlias {
    map: HashMap<String, MsidInfo>,
}

impl MsidAlias {
    pub fn add_alias(&mut self, uuid: &str, label: &str, kind: &str, name: &str) {
        self.map.insert(
            uuid.to_string(),
            MsidInfo {
                label: label.to_string(),
                kind: kind.to_string(),
                name: name.to_string(),
            },
        );
    }

    pub fn get_alias(&self, stream_id: &str, track_id: &str) -> Option<MsidInfo> {
        if let Some(info) = self.map.get(stream_id) {
            return Some(info.clone());
        }

        self.map.get(track_id).cloned()
    }
}

//TODO test this
