use std::{collections::HashMap, sync::Arc, time::Duration};

use media_server_protocol::multi_tenancy::{AppContext, AppId, AppSecret};
use media_server_secure::AppStorage;
use serde::{Deserialize, Serialize};
use spin::rwlock::RwLock;

struct AppSlot {
    app_id: AppId,
}

#[derive(Default)]
pub struct MultiTenancyStorage {
    apps: Arc<RwLock<HashMap<AppSecret, AppSlot>>>,
}

impl MultiTenancyStorage {
    pub fn sync(&self, new_apps: impl Iterator<Item = (String, AppInfo)>) {
        let mut apps = self.apps.write();
        let pre_len = apps.len();
        apps.clear();
        for (app_id, info) in new_apps {
            apps.insert(info.secret.into(), AppSlot { app_id: app_id.into() });
        }
        if pre_len != apps.len() {
            log::info!("[MultiTenancyStorage] updated with {} apps", apps.len());
        }
    }

    pub fn is_empty(&self) -> bool {
        self.apps.read().is_empty()
    }

    pub fn len(&self) -> usize {
        self.apps.read().len()
    }
}

impl AppStorage for MultiTenancyStorage {
    fn validate_app(&self, secret: &str) -> Option<AppContext> {
        let secret: AppSecret = secret.to_owned().into();
        let apps = self.apps.read();
        let slot = apps.get(&secret)?;
        Some(AppContext { app: slot.app_id.clone() })
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub secret: String,
}

#[derive(Serialize, Deserialize)]
pub struct MultiTenancySyncResponse {
    apps: HashMap<String, AppInfo>,
}

pub struct MultiTenancySync {
    storage: Arc<MultiTenancyStorage>,
    endpoint: String,
    interval: Duration,
}

impl MultiTenancySync {
    pub fn new(storage: Arc<MultiTenancyStorage>, endpoint: &str, interval: Duration) -> Self {
        Self {
            storage,
            endpoint: endpoint.to_owned(),
            interval,
        }
    }

    async fn sync(&mut self) -> Result<(), reqwest::Error> {
        let res = reqwest::ClientBuilder::default()
            .timeout(self.interval / 2)
            .build()
            .expect("Should create client")
            .get(&self.endpoint)
            .send()
            .await?
            .error_for_status()?;
        let res_json: MultiTenancySyncResponse = res.json().await?;
        self.storage.sync(res_json.apps.into_iter());
        Ok(())
    }

    pub async fn run_loop(&mut self) {
        log::info!("[MultiTenancySync] start sync");
        loop {
            if let Err(e) = self.sync().await {
                log::error!("[MultiTenancySync] sync error {e:?}");
            }
            tokio::time::sleep(self.interval).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use httpmock::{Mock, MockServer};
    use reqwest::StatusCode;

    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_sync() {
        let storage = Arc::new(MultiTenancyStorage::default());
        let app_info = AppInfo { secret: "secret1".to_string() };
        let new_apps = vec![("app1".to_string(), app_info)];

        storage.sync(new_apps.into_iter());

        assert_eq!(storage.len(), 1);
    }

    #[tokio::test]
    async fn test_validate_app() {
        let storage = Arc::new(MultiTenancyStorage::default());
        let app_info = AppInfo { secret: "secret1".to_string() };
        storage.sync(vec![("app1".to_string(), app_info.clone())].into_iter());

        let context = storage.validate_app("secret1");
        assert_eq!(context, Some(AppContext { app: AppId::from("app1") }));
    }

    fn mockhttp<'a>(server: &'a MockServer, data: Result<&MultiTenancySyncResponse, StatusCode>) -> Mock<'a> {
        // Create a mock on the server.
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/sync");
            match data {
                Ok(data) => {
                    then.status(200).header("content-type", "application/json").json_body_obj(data);
                }
                Err(code) => {
                    then.status(code.as_u16()).body("FAKE_ERROR");
                }
            }
        });

        mock
    }

    #[tokio::test]
    async fn test_sync_ok() {
        // Start a lightweight mock server.
        let server = MockServer::start();
        mockhttp(
            &server,
            Ok(&MultiTenancySyncResponse {
                apps: HashMap::from([("app1".to_owned(), AppInfo { secret: "secret1".to_owned() })]),
            }),
        );

        let storage = Arc::new(MultiTenancyStorage::default());
        let mut sync = MultiTenancySync::new(storage.clone(), &server.url("/sync"), Duration::from_secs(100));
        sync.sync().await.expect("Should sync ok");

        assert_eq!(storage.len(), 1);
        assert_eq!(storage.validate_app("secret1"), Some(AppContext { app: AppId::from("app1") }));
    }

    #[tokio::test]
    async fn test_sync_error() {
        // Start a lightweight mock server.
        let server = MockServer::start();
        mockhttp(&server, Err(StatusCode::BAD_GATEWAY));

        let storage = Arc::new(MultiTenancyStorage::default());
        let mut sync = MultiTenancySync::new(storage.clone(), &server.url("/sync"), Duration::from_secs(100));
        sync.sync().await.expect_err("Should sync error because of http error");

        assert_eq!(storage.len(), 0);
    }
}
