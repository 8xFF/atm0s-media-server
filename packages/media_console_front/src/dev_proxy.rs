//! Poem-proxy is a simple and easy-to-use proxy [Endpoint](poem::Endpoint) compatible with the
//! [Poem Web Framework](poem). It supports the forwarding of http get and post requests
//! as well as websockets right out of the box!
//!
//! # Table of Contents
//!
//! - [Quickstart](#quickstart)
//! - [Proxy Configuration](#proxy-configuration)
//! - [Endpoint](#endpoint)
//!
//! # Quickstart
//!
//! ```
//! use poem::{get, handler, listener::TcpListener, web::Path, IntoResponse, Route, Server, EndpointExt};
//! use media_console_front::dev_proxy::{proxy, ProxyConfig};
//!
//! let pconfig = ProxyConfig::new( "localhost:5173" )
//!     .web_insecure()   // Enables proxy-ing web requests, sets the proxy to use http instead of https
//!     .enable_nesting() // Sets the proxy to support nested routes
//!     .finish();        // Finishes constructing the configuration
//!
//! let app = Route::new().nest( "/", proxy.data( pconfig ) ); // Set the endpoint and pass in the configuration
//!
//! Server::new(TcpListener::bind("127.0.0.1:3000")).run(app); // Start the server
//! ```
//!
//! # Configuration
//!
//! Configuration of this endpoint is done through the
//! [ProxyConfig](ProxyConfig) builder-struct. There are lots of configuration options
//! available, so click that link to learn more about all of them! Below is a brief
//! overview:
//!
//! ```
//! use media_console_front::dev_proxy::ProxyConfig;
//!     
//! // Configure proxy endpoint, pass in the target server address and port number
//! let proxy_config = ProxyConfig::new( "localhost:5173" ) // 5173 is for Sveltekit
//!     
//!     // One of the following lines is required to proxy web requests (post, get, etc)
//!     .web_insecure() // http from proxy to server
//!     .web_secure()   // https from proxy to server
//!
//!     // The following option is required to support nesting
//!     .enable_nesting()
//!
//!     // This returns a concrete ProxyConfig struct to be passed into the endpoint data
//!     .finish();
//! ```
//!
//! # Endpoint
//!
//! This [Endpoint](poem::Endpoint) is a very basic but capable proxy. It works by simply
//! accepting web/socket requests and sending its own request to the target. Then, it
//! sends everything it receives from the target to the connected client.
//!
//! This can be used with poem's built-in routing. You can apply specific request types,
//! or even use [at](poem::Route::at) and [nest](poem::Route::at).
//!
//! The [Quickstart](#quickstart) section shows a working example, so this section doesn't.

use poem::{
    handler,
    http::{Method, StatusCode},
    web::Data,
    Body, Error, Request, Response, Result,
};

/// A configuration object that allows for fine-grained control over a proxy endpoint.
#[derive(Clone, Debug)]
pub struct ProxyConfig {
    /// This is the url where requests and websocket connections are to be
    /// forwarded to. Port numbers are supported here, though they may be
    /// broken off into their own parameter in the future.
    proxy_target: String,

    /// Whether to use https (true) or http for requests to the proxied server. If not
    /// set, the proxy will not forward web requests.
    web_secure: Option<bool>,

    /// Whether or not nesting should be supported when forwarding requests
    /// to the server.
    support_nesting: bool,
}

impl Default for ProxyConfig {
    /// Returns the default value for the [ProxyConfig], which corresponds
    /// to the following:
    /// > `proxy_target: "http://localhost:3000"`
    ///
    /// > `web_secure: None`
    ///
    /// > `ws_secure: None`
    ///
    /// > `support_nesting: false`
    fn default() -> Self {
        Self {
            proxy_target: "http://localhost:3000".into(),
            web_secure: None,
            support_nesting: false,
        }
    }
}

/// # Implementation of Builder Functions
///
/// The ProxyConfig struct follows the builder pattern to enable explicit
/// and succinct configuration of the proxy endpoint.
#[allow(unused)]
impl ProxyConfig {
    /// Function that creates a new ProxyConfig for a given target
    /// and sets all other parameters to their default values. See
    /// [the default implementation](ProxyConfig::default) for more
    /// information.
    pub fn new(target: impl Into<String>) -> ProxyConfig {
        ProxyConfig {
            proxy_target: target.into(),
            ..ProxyConfig::default()
        }
    }

    /// This function sets the endpoint to forward requests to the
    /// target over the https protocol. This is a secure and encrypted
    /// communication channel that should be utilized when possible.
    pub fn web_secure(&mut self) -> &mut ProxyConfig {
        self.web_secure = Some(true);
        self
    }

    /// This function sets the endpoint to forward requests to the
    /// target over the http protocol. This is an insecure and unencrypted
    /// communication channel that should be used very carefully.
    pub fn web_insecure(&mut self) -> &mut ProxyConfig {
        self.web_secure = Some(false);
        self
    }

    /// This function sets the waypoint to support nesting.
    ///
    /// For example,
    /// if `endpoint.target` is `https://google.com` and the proxy is reached
    /// at `https://proxy_address/favicon.png`, the proxy server will forward
    /// the request to `https://google.com/favicon.png`.
    pub fn enable_nesting(&mut self) -> &mut ProxyConfig {
        self.support_nesting = true;
        self
    }

    /// This function sets the waypoint to ignore nesting.
    ///
    /// For example,
    /// if `endpoint.target` is `https://google.com` and the proxy is reached
    /// at `https://proxy_address/favicon.png`, the proxy server will forward
    /// the request to `https://google.com`.
    pub fn disable_nesting(&mut self) -> &mut ProxyConfig {
        self.support_nesting = false;
        self
    }

    /// Finishes off the building process by returning a new ProxyConfig object
    /// (not reference) that contains all the settings that were previously
    /// specified.
    pub fn finish(&mut self) -> ProxyConfig {
        self.clone()
    }
}

/// # Convenience Functions
///
/// These functions make it possible to get information from the ProxyConfig struct.
impl ProxyConfig {
    /// Returns the target url of the request, including the proper protocol information
    /// and the correct pathing if nesting is enabled
    ///
    /// An example output would be
    ///
    /// > `"https://proxy.domain.com"`
    pub fn get_web_request_uri(&self, subpath: Option<String>) -> Result<String, ()> {
        let Some(secure) = self.web_secure else {
            return Err(());
        };

        let base = if secure {
            format!("https://{}", self.proxy_target)
        } else {
            format!("http://{}", self.proxy_target)
        };

        let sub = self.support_nesting.then_some(subpath).flatten().unwrap_or_default();

        println!("base: {} | sub: {}", base, sub);

        Ok(base + &sub)
    }
}

/// The websocket-enabled proxy handler
#[handler]
pub async fn proxy(req: &Request, config: Data<&ProxyConfig>, method: Method, body: Body) -> Result<Response> {
    // Update the uri to point to the proxied server
    // let request_uri = target.to_owned() + &req.uri().to_string();

    // Get the websocket URI if websockets are supported, otherwise return an error
    let Ok(uri) = config.get_web_request_uri(Some(req.uri().to_string())) else {
        return Err(Error::from_string("Proxy endpoint not configured to support web requests!", StatusCode::NOT_IMPLEMENTED));
    };

    // Now generate a request for the proxied server, based on information
    // that we have from the current request
    let client = reqwest::Client::new();
    let res = match method {
        Method::GET => client.get(uri).headers(req.headers().clone()).body(body.into_bytes().await.unwrap()).send().await,
        Method::POST => client.post(uri).headers(req.headers().clone()).body(body.into_bytes().await.unwrap()).send().await,
        _ => {
            return Err(Error::from_string(
                "Unsupported Method! The proxy endpoint currently only supports GET and POST requests!",
                StatusCode::METHOD_NOT_ALLOWED,
            ))
        }
    };

    // Check on the response and forward everything from the server to our client,
    // including headers and the body of the response, among other things.
    match res {
        Ok(result) => {
            let mut res = Response::default();
            res.extensions().clone_from(&result.extensions());
            result.headers().iter().for_each(|(key, val)| {
                res.headers_mut().insert(key, val.to_owned());
            });
            res.set_status(result.status());
            res.set_version(result.version());
            res.set_body(result.bytes().await.unwrap());
            Ok(res)
        }

        // The request to the back-end server failed. Why?
        Err(error) => Err(Error::from_string(error.to_string(), error.status().unwrap_or(StatusCode::BAD_GATEWAY))),
    }
}
