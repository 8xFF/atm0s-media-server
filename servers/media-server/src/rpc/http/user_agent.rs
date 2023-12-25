use poem::{http::StatusCode, FromRequest, Request, RequestBody, Result};

#[derive(Debug)]
pub struct UserAgent(pub String);

#[poem::async_trait]
impl<'a> FromRequest<'a> for UserAgent {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> Result<Self> {
        let headers = req.headers();
        let user_agent = headers.get("User-Agent").ok_or(poem::Error::from_string("Bad Request", StatusCode::BAD_REQUEST))?;
        let user_agent = user_agent.to_str().map_err(|_| poem::Error::from_string("Bad Request", StatusCode::BAD_REQUEST))?;
        Ok(UserAgent(user_agent.into()))
    }
}
