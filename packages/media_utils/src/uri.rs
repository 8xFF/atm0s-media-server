use serde::de::DeserializeOwned;
use serde_querystring::DuplicateQS;

pub struct CustomUri<Q> {
    pub username: Option<String>,
    pub password: Option<String>,
    pub endpoint: String,
    pub path: Vec<String>,
    pub query: Q,
}

impl<Q: DeserializeOwned> TryFrom<&str> for CustomUri<Q> {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match uriparse::URIReference::try_from(value) {
            Ok(uri) => match uri.scheme() {
                Some(uriparse::Scheme::HTTP) | Some(uriparse::Scheme::HTTPS) => {
                    let path = uri.path().segments().iter().map(|s| s.to_string()).collect::<Vec<_>>();
                    let is_https = matches!(uri.scheme(), Some(uriparse::Scheme::HTTPS));
                    let query: Q = DuplicateQS::parse(uri.query().map(|q| q.as_bytes()).unwrap_or(b"")).deserialize().map_err(|_| "WRONG_QUERY_SCHEMA")?;
                    let host = uri.host().ok_or("MISSING_HOST")?;
                    let endpoint = match (is_https, uri.port()) {
                        (true, Some(443)) => format!("https://{}", host),
                        (false, Some(80)) => format!("http://{}", host),
                        (true, None) => format!("https://{}", host),
                        (false, None) => format!("http://{}", host),
                        (true, Some(port)) => format!("https://{}:{}", host, port),
                        (false, Some(port)) => format!("http://{}:{}", host, port),
                    };

                    Ok(Self {
                        username: uri.username().map(|u| u.to_string()),
                        password: uri.password().map(|u| u.to_string()),
                        endpoint,
                        path,
                        query,
                    })
                }
                _ => Err("WRONG_SCHEME"),
            },
            Err(_err) => Err("WRONG_URI"),
        }
    }
}
