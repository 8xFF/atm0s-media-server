use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[cfg(test)]
mod test {
    #[test]
    fn should_work() {
        let mut alias = super::MsidAlias::default();
        alias.add_alias("uuid", "label", "kind", "name");
        assert_eq!(
            alias.get_alias("uuid", "track_id"),
            Some(super::MsidInfo {
                label: "label".to_string(),
                kind: "kind".to_string(),
                name: "name".to_string(),
            })
        );
        assert_eq!(
            alias.get_alias("stream_id", "uuid"),
            Some(super::MsidInfo {
                label: "label".to_string(),
                kind: "kind".to_string(),
                name: "name".to_string(),
            })
        );
    }
}
