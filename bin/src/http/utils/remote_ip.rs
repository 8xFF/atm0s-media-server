use std::{net::IpAddr, ops::Deref};

use poem::{http::StatusCode, FromRequest};

#[derive(Debug)]
pub struct RemoteIpAddr(pub IpAddr);

impl<'a> FromRequest<'a> for RemoteIpAddr {
    async fn from_request(req: &'a poem::Request, _body: &mut poem::RequestBody) -> poem::Result<Self> {
        let headers = req.headers();
        if let Some(remote_addr) = headers.get("X-Forwarded-For") {
            let remote_addr = remote_addr.to_str().map_err(|_| poem::Error::from_string("Bad Request", StatusCode::BAD_REQUEST))?;
            let remote_addr = remote_addr.split(',').next().ok_or(poem::Error::from_string("Bad Request", StatusCode::BAD_REQUEST))?;
            Ok(RemoteIpAddr(remote_addr.parse().map_err(|_| poem::Error::from_string("Invalid IP address", StatusCode::BAD_REQUEST))?))
        } else if let Some(remote_addr) = headers.get("X-Real-IP") {
            let remote_addr = remote_addr.to_str().map_err(|_| poem::Error::from_string("Bad Request", StatusCode::BAD_REQUEST))?;
            Ok(RemoteIpAddr(remote_addr.parse().map_err(|_| poem::Error::from_string("Invalid IP address", StatusCode::BAD_REQUEST))?))
        } else {
            match req.remote_addr().deref() {
                poem::Addr::SocketAddr(addr) => Ok(RemoteIpAddr(addr.ip())),
                _ => Err(poem::Error::from_string("Bad Request", StatusCode::BAD_REQUEST)),
            }
        }
    }
}
