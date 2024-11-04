use std::collections::BTreeMap;

use media_server_utils::get_all_counts;
use poem_openapi::{payload::Json, OpenApi};

pub struct Apis;

#[OpenApi]
impl Apis {
    #[oai(path = "/counts", method = "get")]
    async fn get_counts(&self) -> Json<BTreeMap<String, usize>> {
        Json(get_all_counts().into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
}
