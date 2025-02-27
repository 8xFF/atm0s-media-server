use std::ops::{Deref, DerefMut};

use poem::{FromRequest, IntoResponse, Request, RequestBody, Response, Result};
use poem_openapi::{
    impl_apirequest_for_payload,
    payload::{ParsePayload, Payload},
    registry::{MetaMediaType, MetaResponse, MetaResponses, MetaSchema, MetaSchemaRef, Registry},
    ApiResponse,
};

/// A ProtoBuffer payload.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Protobuf<T>(pub T);

impl<T> Deref for Protobuf<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Protobuf<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: prost::Message> Payload for Protobuf<T> {
    const CONTENT_TYPE: &'static str = "application/grpc";

    fn check_content_type(_content_type: &str) -> bool {
        true
    }

    fn schema_ref() -> MetaSchemaRef {
        MetaSchemaRef::Inline(Box::new(MetaSchema {
            format: Some("binary"),
            description: Some("ProtoBuffer"),
            example: Some(std::any::type_name::<T>().into()),
            ..MetaSchema::new(std::any::type_name::<T>())
        }))
    }
}

impl<T: Default + prost::Message> ParsePayload for Protobuf<T> {
    const IS_REQUIRED: bool = true;

    async fn from_request(request: &Request, body: &mut RequestBody) -> Result<Self> {
        let data = Vec::<u8>::from_request(request, body).await?;
        let value = T::decode(data.as_slice()).unwrap();
        Ok(Self(value))
    }
}

impl<T: prost::Message> IntoResponse for Protobuf<T> {
    fn into_response(self) -> Response {
        self.0.encode_to_vec().into_response()
    }
}

impl<T: prost::Message> ApiResponse for Protobuf<T> {
    fn meta() -> MetaResponses {
        MetaResponses {
            responses: vec![MetaResponse {
                description: "ProtoBuffer",
                status: Some(200),
                content: vec![MetaMediaType {
                    content_type: Self::CONTENT_TYPE,
                    schema: Self::schema_ref(),
                }],
                headers: vec![],
                status_range: None,
            }],
        }
    }

    fn register(_registry: &mut Registry) {}
}

impl_apirequest_for_payload!(Protobuf<T>, T: Default + prost::Message);
