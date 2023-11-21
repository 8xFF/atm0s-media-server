use std::ops::{Deref, DerefMut};

use poem::{
    http::{HeaderValue, StatusCode},
    FromRequest, IntoResponse, Request, RequestBody, Response, Result,
};

use poem_openapi::{
    impl_apirequest_for_payload,
    payload::{ParsePayload, Payload},
    registry::{MetaMediaType, MetaResponse, MetaResponses, MetaSchemaRef, Registry},
    types::Type,
    ApiResponse,
};

/// A UTF8 string payload.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ApplicationSdp<T>(pub T);

impl<T> Deref for ApplicationSdp<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for ApplicationSdp<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Send> Payload for ApplicationSdp<T> {
    const CONTENT_TYPE: &'static str = "application/sdp";

    fn check_content_type(content_type: &str) -> bool {
        //TODO: check content type
        return true;
    }

    fn schema_ref() -> MetaSchemaRef {
        String::schema_ref()
    }
}

#[poem::async_trait]
impl ParsePayload for ApplicationSdp<String> {
    const IS_REQUIRED: bool = true;

    async fn from_request(request: &Request, body: &mut RequestBody) -> Result<Self> {
        Ok(Self(String::from_request(request, body).await?))
    }
}

impl<T: Into<String> + Send> IntoResponse for ApplicationSdp<T> {
    fn into_response(self) -> Response {
        self.0.into().into_response()
    }
}

impl<T: Into<String> + Send> ApiResponse for ApplicationSdp<T> {
    fn meta() -> MetaResponses {
        MetaResponses {
            responses: vec![MetaResponse {
                description: "",
                status: Some(200),
                content: vec![MetaMediaType {
                    content_type: Self::CONTENT_TYPE,
                    schema: Self::schema_ref(),
                }],
                headers: vec![],
            }],
        }
    }

    fn register(_registry: &mut Registry) {}
}

pub struct HttpResponse<T: IntoResponse> {
    pub code: StatusCode,
    pub res: T,
    pub headers: Vec<(&'static str, String)>,
}

impl<T: IntoResponse> IntoResponse for HttpResponse<T> {
    fn into_response(self) -> Response {
        let mut res = self.res.into_response();
        for (k, v) in self.headers {
            if let Ok(v) = HeaderValue::from_str(&v) {
                res.headers_mut().insert(k, v);
            }
        }
        res.set_status(self.code);
        res
    }
}

impl<T: Payload + IntoResponse> ApiResponse for HttpResponse<T> {
    fn meta() -> MetaResponses {
        MetaResponses {
            responses: vec![MetaResponse {
                description: "",
                status: Some(200),
                content: vec![MetaMediaType {
                    content_type: T::CONTENT_TYPE,
                    schema: T::schema_ref(),
                }],
                headers: vec![],
            }],
        }
    }

    fn register(_registry: &mut Registry) {}
}

impl_apirequest_for_payload!(ApplicationSdp<String>);
