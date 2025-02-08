use std::{path::PathBuf, sync::Arc, time::Duration};

use atm0s_media_server_record::{
    convert::{RecordComposerConfig, RecordConvert, RecordConvertConfig, RecordConvertOutputLocation},
    convert_s3_uri,
};
use clap::Parser;
use media_server_connector::{hooks::ConnectorHookSender, HookBodyType};
use media_server_multi_tenancy::{MultiTenancyStorage, MultiTenancySync};
use media_server_protocol::protobuf::cluster_connector::{
    compose_event::{self, record_job_completed::ComposeSummary, RecordJobCompleted, RecordJobFailed},
    hook_event, ComposeEvent, HookEvent,
};
use media_server_secure::AppStorage;
use media_server_utils::now_ms;
use poem::{
    listener::TcpListener,
    middleware::{Cors, Tracing},
    EndpointExt, Route,
};
use poem_openapi::{auth::Bearer, SecurityScheme};
use poem_openapi::{
    payload::Json,
    types::{ParseFromJSON, ToJSON, Type},
    Object, OpenApi, OpenApiService,
};
use rusty_s3::S3Action;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

macro_rules! try_validate_app {
    ($self:ident, $token:ident, $err:expr) => {
        match $self.apps.validate_app(&$token.token) {
            Some(app) => app,
            None => {
                return Json(Response {
                    status: false,
                    error: Some($err.to_owned()),
                    data: None,
                })
            }
        }
    };
}

macro_rules! try_opt {
    ($opt:expr, $err:expr) => {
        match $opt {
            Some(o) => o,
            None => {
                return Json(Response {
                    status: false,
                    error: Some($err.to_owned()),
                    data: None,
                });
            }
        }
    };
}

#[derive(SecurityScheme)]
#[oai(rename = "Token Authorization", ty = "bearer", key_in = "header", key_name = "Authorization")]
pub struct TokenAuthorization(pub Bearer);

/// Record convert worker for atm0s-media-server.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Http listen address
    #[arg(env, long, default_value = "0.0.0.0:3200")]
    http_addr: String,

    /// S3 uri for raw file
    #[arg(env, long)]
    input_s3_uri: String,

    /// S3 uri for transmux file
    #[arg(env, long)]
    transmux_s3_uri: Option<String>,

    /// S3 uri for composed file
    #[arg(env, long)]
    compose_s3_uri: Option<String>,

    /// Hook Uri.
    /// If set, will send hook event to this uri. example: http://localhost:8080/hook
    #[arg(env, long)]
    hook_uri: Option<String>,

    /// Hook workers
    #[arg(env, long, default_value_t = 8)]
    hook_workers: usize,

    /// Hook body type
    #[arg(env, long, default_value = "protobuf-json")]
    hook_body_type: HookBodyType,

    /// multi-tenancy sync endpoint
    #[arg(env, long)]
    multi_tenancy_sync: Option<String>,

    /// multi-tenancy sync endpoint
    #[arg(env, long, default_value_t = 30_000)]
    multi_tenancy_sync_interval_ms: u64,

    /// Cluster secret key used for secure communication between nodes.
    #[arg(env, long, default_value = "insecure")]
    secret: String,
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info");
    }
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }
    let args: Args = Args::parse();
    tracing_subscriber::registry().with(fmt::layer()).with(EnvFilter::from_default_env()).init();

    let apps = if let Some(url) = args.multi_tenancy_sync {
        let apps = Arc::new(MultiTenancyStorage::new());
        let mut sync = MultiTenancySync::new(apps.clone(), url, Duration::from_millis(args.multi_tenancy_sync_interval_ms));
        tokio::spawn(async move {
            sync.run_loop().await;
        });
        apps
    } else {
        Arc::new(MultiTenancyStorage::new_with_single(args.secret.as_str(), args.hook_uri.as_deref()))
    };

    let hook = Arc::new(ConnectorHookSender::new(args.hook_workers, args.hook_body_type, apps.clone()));

    let apis = OpenApiService::new(
        HttpApis {
            apps,
            hook,
            transmux_s3_uri: args.transmux_s3_uri.unwrap_or_else(|| args.input_s3_uri.clone()),
            compose_s3_uri: args.compose_s3_uri.unwrap_or_else(|| args.input_s3_uri.clone()),
            input_s3_uri: args.input_s3_uri,
        },
        "Convert Worker APIs",
        env!("CARGO_PKG_VERSION"),
    )
    .server("/api/");
    let apis_ui = apis.swagger_ui();
    let apis_spec = apis.spec();

    let app = Route::new()
        .nest("/api/", apis)
        .nest("/api/docs", apis_ui)
        .at("/api/spec", poem::endpoint::make_sync(move |_| apis_spec.clone()))
        .with(Cors::new())
        .with(Tracing);

    log::info!("Starting convert worker on {}", args.http_addr);
    poem::Server::new(TcpListener::bind(args.http_addr)).run(app).await
}

#[derive(Debug, Object)]
struct TransmuxConfig {
    custom_s3: Option<String>,
}

#[derive(Debug, Object)]
struct ComposeConfig {
    audio: bool,
    video: bool,
    custom_s3: Option<String>,
}

#[derive(Debug, Object)]
struct ConvertJobRequest {
    record_path: String,
    transmux: Option<TransmuxConfig>,
    compose: Option<ComposeConfig>,
}

#[derive(Debug, Object)]
struct ConvertJobResponse {
    job_id: String,
}

#[derive(Debug, Object)]
pub struct Response<T: ParseFromJSON + ToJSON + Type + Send + Sync> {
    pub status: bool,
    #[oai(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[oai(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

struct HttpApis {
    apps: Arc<MultiTenancyStorage>,
    hook: Arc<ConnectorHookSender>,
    input_s3_uri: String,
    transmux_s3_uri: String,
    compose_s3_uri: String,
}

#[OpenApi]
impl HttpApis {
    #[oai(path = "/convert/job", method = "post")]
    async fn create_job(&self, TokenAuthorization(token): TokenAuthorization, Json(body): Json<ConvertJobRequest>) -> Json<Response<ConvertJobResponse>> {
        let app = try_validate_app!(self, token, "Invalid token");
        let job_id = rand::random::<u64>().to_string();
        let input_s3 = try_opt!(concat_s3_uri_path(&self.input_s3_uri, &body.record_path), "Invalid input_s3_uri");
        let compose_s3_uri = self.compose_s3_uri.clone();
        let job_id_c = job_id.clone();
        let hook = self.hook.clone();

        // get yyyy/mm/dd with chrono
        let current_date_path = chrono::Utc::now().format("%Y/%m/%d").to_string();
        let transmux = if let Some(t) = body.transmux {
            if let Some(custom_s3) = t.custom_s3 {
                Some(RecordConvertOutputLocation::S3(custom_s3))
            } else {
                let s3 = try_opt!(
                    concat_s3_uri_path(&self.transmux_s3_uri, &format!("{}/transmux/{current_date_path}/{job_id_c}", app.app)),
                    "Invalid transmux_s3_uri"
                );
                Some(RecordConvertOutputLocation::S3(s3))
            }
        } else {
            None
        };

        tokio::spawn(async move {
            log::info!("Convert job {job_id_c} started");
            hook.on_event(
                app.app.clone(),
                HookEvent {
                    node: 0,
                    ts: now_ms(),
                    event: Some(hook_event::Event::Compose(ComposeEvent {
                        app: app.app.clone().into(),
                        job_id: job_id_c.clone(),
                        event: Some(compose_event::Event::Started(Default::default())),
                    })),
                },
            );

            let converter = RecordConvert::new(RecordConvertConfig {
                in_s3: input_s3,
                transmux,
                compose: body.compose.map(|c| {
                    let (uri, relative) = c
                        .custom_s3
                        .map(|u| {
                            let relative = u.split('?').collect::<Vec<_>>()[0].to_string();
                            (u, relative)
                        })
                        .unwrap_or_else(|| {
                            let (s3, credentials, s3_sub_folder) = convert_s3_uri(&compose_s3_uri).expect("should convert compose_s3_uri");
                            let relative = format!("{}/compose/{current_date_path}/{job_id_c}.webm", app.app);
                            let path = PathBuf::from(s3_sub_folder).join(&relative);
                            let put = s3.put_object(Some(&credentials), path.to_str().expect("should convert to path"));
                            let uri = put.sign(Duration::from_secs(3600)).to_string();
                            (uri, relative)
                        });
                    RecordComposerConfig {
                        audio: c.audio,
                        video: c.video,
                        output_relative: relative,
                        output: RecordConvertOutputLocation::S3(uri),
                    }
                }),
            });
            let result = match converter.convert().await {
                Ok(summary) => {
                    log::info!("Convert job {job_id_c} completed");
                    compose_event::Event::Completed(RecordJobCompleted {
                        transmux: summary.transmux.map(|s| s.into()),
                        compose: summary.compose.map(|s| ComposeSummary {
                            media_uri: s.media_uri,
                            duration_ms: s.duration_ms,
                        }),
                    })
                }
                Err(e) => {
                    log::error!("Convert job {job_id_c} failed: {e}");
                    compose_event::Event::Failed(RecordJobFailed { error: e.to_string() })
                }
            };

            hook.on_event(
                app.app.clone(),
                HookEvent {
                    node: 0,
                    ts: now_ms(),
                    event: Some(hook_event::Event::Compose(ComposeEvent {
                        app: app.app.into(),
                        job_id: job_id_c,
                        event: Some(result),
                    })),
                },
            );
        });
        Json(Response {
            status: true,
            error: None,
            data: Some(ConvertJobResponse { job_id }),
        })
    }
}

fn concat_s3_uri_path(s3_uri: &str, path: &str) -> Option<String> {
    fn ensure_last_slash(s: String) -> String {
        if s.ends_with('/') {
            s
        } else {
            s + "/"
        }
    }

    let parts = s3_uri.split('?').collect::<Vec<_>>();
    if parts.len() == 2 {
        let first = PathBuf::from(parts[0]).join(path).to_str()?.to_string();
        let first = ensure_last_slash(first);
        log::info!("first: {}", first);
        Some(first + "?" + parts[1])
    } else {
        let first = PathBuf::from(s3_uri).join(path).to_str()?.to_string();
        let first = ensure_last_slash(first);
        log::info!("first: {}", first);
        Some(first)
    }
}
