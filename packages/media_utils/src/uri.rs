use serde::de::DeserializeOwned;
use serde_querystring::DuplicateQS;

#[derive(Debug, Clone)]
pub struct CustomUri<Q> {
    pub username: Option<String>,
    pub password: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    pub tls: bool,
    pub path: Vec<String>,
    pub query: Q,
}

impl<Q> CustomUri<Q> {
    pub fn root(&self) -> String {
        match self.port {
            Some(port) => format!("{}:{}", self.host, port),
            None => self.host.clone(),
        }
    }

    pub fn endpoint_url(&self) -> String {
        let scheme = if self.tls {
            "https"
        } else {
            "http"
        };
        format!("{}://{}", scheme, self.root())
    }
}

impl<Q: DeserializeOwned> TryFrom<&str> for CustomUri<Q> {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match uriparse::URIReference::try_from(value) {
            Ok(uri) => match uri.scheme() {
                Some(uriparse::Scheme::HTTP) | Some(uriparse::Scheme::HTTPS) => {
                    let path = uri.path().segments().iter().map(|s| s.to_string()).collect::<Vec<_>>();
                    let tls = matches!(uri.scheme(), Some(uriparse::Scheme::HTTPS));
                    let query: Q = DuplicateQS::parse(uri.query().map(|q| q.as_bytes()).unwrap_or(b"")).deserialize().map_err(|_| "WRONG_QUERY_SCHEMA")?;
                    let host = uri.host().ok_or("MISSING_HOST")?;
                    let port = match (tls, uri.port()) {
                        (true, Some(443)) | (false, Some(80)) => None,
                        (_, port) => port,
                    };

                    let username = uri.username().and_then(|u| urlencoding::decode(u.as_ref()).map(|u| u.to_string()).ok());
                    let password = uri.password().and_then(|u| urlencoding::decode(u.as_ref()).map(|u| u.to_string()).ok());

                    Ok(Self {
                        username: username.map(|u| u.to_string()),
                        password: password.map(|u| u.to_string()),
                        host: host.to_string(),
                        port,
                        tls,
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

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Options {
        path_style: Option<bool>,
        region: Option<String>,
    }

    #[test]
    fn parses_http_uri_with_custom_port() {
        let uri = CustomUri::<Options>::try_from("http://access:secret@minio.local:9000/bucket/prefix?path_style=true").unwrap();

        assert_eq!(uri.username.as_deref(), Some("access"));
        assert_eq!(uri.password.as_deref(), Some("secret"));
        assert_eq!(uri.host, "minio.local");
        assert_eq!(uri.port, Some(9000));
        assert!(!uri.tls);
        assert_eq!(uri.root(), "minio.local:9000");
        assert_eq!(uri.endpoint_url(), "http://minio.local:9000");
        assert_eq!(uri.path, vec!["bucket", "prefix"]);
        assert_eq!(uri.query.path_style, Some(true));
    }

    #[test]
    fn parses_https_uri_without_port() {
        let uri = CustomUri::<Options>::try_from("https://access:secret@s3.amazonaws.com/bucket?region=us-east-1").unwrap();

        assert_eq!(uri.host, "s3.amazonaws.com");
        assert_eq!(uri.port, None);
        assert!(uri.tls);
        assert_eq!(uri.root(), "s3.amazonaws.com");
        assert_eq!(uri.endpoint_url(), "https://s3.amazonaws.com");
        assert_eq!(uri.query.region.as_deref(), Some("us-east-1"));
    }

    #[test]
    fn normalizes_default_ports() {
        let https = CustomUri::<Options>::try_from("https://access:secret@example.com:443/bucket").unwrap();
        let http = CustomUri::<Options>::try_from("http://access:secret@example.com:80/bucket").unwrap();

        assert_eq!(https.port, None);
        assert_eq!(https.root(), "example.com");
        assert_eq!(https.endpoint_url(), "https://example.com");
        assert_eq!(http.port, None);
        assert_eq!(http.root(), "example.com");
        assert_eq!(http.endpoint_url(), "http://example.com");
    }
}
