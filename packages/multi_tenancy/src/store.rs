use std::{collections::HashMap, ops::Deref, sync::Arc, time::Duration};

use media_server_protocol::multi_tenancy::{AppContext, AppId, AppSecret};
use media_server_secure::AppStorage;
use serde::{Deserialize, Serialize};
use spin::rwlock::RwLock;

pub struct MultiTenancyStorage {
    internal: RwLock<MultiTenancyStorageInternal>,
}

impl MultiTenancyStorage {
    pub fn new(secret: &str, hook: Option<&str>) -> Self {
        Self {
            internal: RwLock::new(MultiTenancyStorageInternal::new(secret, hook)),
        }
    }

    pub fn sync(&self, new_apps: impl Iterator<Item = AppInfo>) {
        self.internal.write().sync(new_apps);
    }

    pub fn get_app(&self, app: &AppId) -> Option<AppInfo> {
        self.internal.read().get_app(app)
    }

    pub fn is_empty(&self) -> bool {
        self.internal.read().is_empty()
    }

    pub fn len(&self) -> usize {
        self.internal.read().len()
    }
}

impl AppStorage for MultiTenancyStorage {
    fn validate_app(&self, secret: &str) -> Option<AppContext> {
        let secret: AppSecret = secret.to_owned().into();
        let apps = self.internal.read().get_secret(&secret)?;
        Some(AppContext { app: apps.app_id.into() })
    }
}

struct MultiTenancyStorageInternal {
    root_app: AppInfo,
    secrets: HashMap<AppSecret, AppInfo>,
    apps: HashMap<AppId, AppInfo>,
}

impl MultiTenancyStorageInternal {
    pub fn new(secret: &str, hook: Option<&str>) -> Self {
        Self {
            secrets: Default::default(),
            apps: Default::default(),
            root_app: AppInfo {
                app_id: AppId::root_app().into(),
                app_secret: secret.to_owned(),
                hook: hook.map(|h| h.to_owned()),
            },
        }
    }

    fn sync(&mut self, new_apps: impl Iterator<Item = AppInfo>) {
        let pre_len = self.apps.len();
        self.apps.clear();
        self.secrets.clear();
        for info in new_apps {
            self.apps.insert(info.app_id.clone().into(), info.clone());
            self.secrets.insert(info.app_secret.clone().into(), info);
        }
        if pre_len != self.apps.len() {
            log::info!("[MultiTenancyStorage] updated with {} apps", self.apps.len());
        }
    }

    fn get_app(&self, app: &AppId) -> Option<AppInfo> {
        if app.deref().is_empty() {
            Some(self.root_app.clone())
        } else {
            self.apps.get(app).cloned()
        }
    }

    fn get_secret(&self, secret: &AppSecret) -> Option<AppInfo> {
        if self.root_app.app_secret.eq(secret.deref()) {
            return Some(self.root_app.clone());
        }
        self.secrets.get(secret).cloned()
    }

    fn is_empty(&self) -> bool {
        self.apps.is_empty()
    }

    fn len(&self) -> usize {
        self.apps.len()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub app_id: String,
    pub app_secret: String,
    pub hook: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct MultiTenancySyncResponse {
    apps: Vec<AppInfo>,
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
        let storage = Arc::new(MultiTenancyStorage::new("aaaa", None));
        let app_info = AppInfo {
            app_id: "app1".to_owned(),
            app_secret: "secret1".to_string(),
            hook: None,
        };
        let new_apps = vec![app_info];
        storage.sync(new_apps.into_iter());

        assert_eq!(storage.len(), 1);
    }

    #[tokio::test]
    async fn test_validate_app() {
        let storage = Arc::new(MultiTenancyStorage::new("aaaa", None));
        let app_info = AppInfo {
            app_id: "app1".to_owned(),
            app_secret: "secret1".to_string(),
            hook: None,
        };
        let new_apps = vec![app_info];
        storage.sync(new_apps.into_iter());

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
                apps: vec![AppInfo {
                    app_id: "app1".to_owned(),
                    app_secret: "secret1".to_string(),
                    hook: None,
                }],
            }),
        );

        let storage = Arc::new(MultiTenancyStorage::new("aaaa", None));
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

        let storage = Arc::new(MultiTenancyStorage::new("aaaa", None));
        let mut sync = MultiTenancySync::new(storage.clone(), &server.url("/sync"), Duration::from_secs(100));
        sync.sync().await.expect_err("Should sync error because of http error");

        assert_eq!(storage.len(), 0);
    }
}
