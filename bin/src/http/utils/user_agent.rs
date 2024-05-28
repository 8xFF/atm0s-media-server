use poem::{http::StatusCode, FromRequest};

#[derive(Debug)]
pub struct UserAgent(pub String);

impl<'a> FromRequest<'a> for UserAgent {
    async fn from_request(req: &'a poem::Request, _body: &mut poem::RequestBody) -> poem::Result<Self> {
        let headers = req.headers();
        let user_agent = headers.get("User-Agent").ok_or(poem::Error::from_string("Bad Request", StatusCode::BAD_REQUEST))?;
        let user_agent = user_agent.to_str().map_err(|_| poem::Error::from_string("Bad Request", StatusCode::BAD_REQUEST))?;
        Ok(UserAgent(user_agent.into()))
    }
}
