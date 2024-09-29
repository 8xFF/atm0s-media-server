// This file is @generated by prost-build.
#[derive(serde::Serialize)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Config {
    #[prost(message, optional, tag = "1")]
    pub mixer: ::core::option::Option<mixer::Config>,
}
#[derive(serde::Serialize)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Request {
    #[prost(oneof = "request::Request", tags = "1")]
    pub request: ::core::option::Option<request::Request>,
}
/// Nested message and enum types in `Request`.
pub mod request {
    #[derive(serde::Serialize)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Request {
        #[prost(message, tag = "1")]
        Mixer(super::mixer::Request),
    }
}
#[derive(serde::Serialize)]
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct Response {
    #[prost(oneof = "response::Response", tags = "1")]
    pub response: ::core::option::Option<response::Response>,
}
/// Nested message and enum types in `Response`.
pub mod response {
    #[derive(serde::Serialize)]
    #[derive(Clone, Copy, PartialEq, ::prost::Oneof)]
    pub enum Response {
        #[prost(message, tag = "1")]
        Mixer(super::mixer::Response),
    }
}
#[derive(serde::Serialize)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServerEvent {
    #[prost(oneof = "server_event::Event", tags = "1")]
    pub event: ::core::option::Option<server_event::Event>,
}
/// Nested message and enum types in `ServerEvent`.
pub mod server_event {
    #[derive(serde::Serialize)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Event {
        #[prost(message, tag = "1")]
        Mixer(super::mixer::ServerEvent),
    }
}
