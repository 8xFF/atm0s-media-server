// This file is @generated by prost-build.
#[derive(serde::Serialize)]
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct Pagination {
    #[prost(uint32, tag = "1")]
    pub total: u32,
    #[prost(uint32, tag = "2")]
    pub current: u32,
}
#[derive(serde::Serialize)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Error {
    #[prost(uint32, tag = "1")]
    pub code: u32,
    #[prost(string, tag = "2")]
    pub message: ::prost::alloc::string::String,
}
#[derive(serde::Serialize)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Receiver {
    #[prost(enumeration = "Kind", tag = "1")]
    pub kind: i32,
    #[prost(string, tag = "2")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub state: ::core::option::Option<receiver::State>,
}
/// Nested message and enum types in `Receiver`.
pub mod receiver {
    #[derive(serde::Serialize)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Source {
        #[prost(string, tag = "1")]
        pub peer: ::prost::alloc::string::String,
        #[prost(string, tag = "2")]
        pub track: ::prost::alloc::string::String,
    }
    #[derive(serde::Serialize)]
    #[derive(Clone, Copy, PartialEq, ::prost::Message)]
    pub struct Config {
        #[prost(uint32, tag = "1")]
        pub priority: u32,
        #[prost(uint32, tag = "2")]
        pub max_spatial: u32,
        #[prost(uint32, tag = "3")]
        pub max_temporal: u32,
        #[prost(uint32, optional, tag = "4")]
        pub min_spatial: ::core::option::Option<u32>,
        #[prost(uint32, optional, tag = "5")]
        pub min_temporal: ::core::option::Option<u32>,
    }
    #[derive(serde::Serialize)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct State {
        #[prost(message, optional, tag = "1")]
        pub config: ::core::option::Option<Config>,
        #[prost(message, optional, tag = "2")]
        pub source: ::core::option::Option<Source>,
    }
    #[derive(serde::Serialize)]
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Status {
        Waiting = 0,
        Active = 1,
        Inactive = 2,
    }
    impl Status {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Self::Waiting => "WAITING",
                Self::Active => "ACTIVE",
                Self::Inactive => "INACTIVE",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "WAITING" => Some(Self::Waiting),
                "ACTIVE" => Some(Self::Active),
                "INACTIVE" => Some(Self::Inactive),
                _ => None,
            }
        }
    }
}
#[derive(serde::Serialize)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Sender {
    #[prost(enumeration = "Kind", tag = "1")]
    pub kind: i32,
    #[prost(string, tag = "2")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub state: ::core::option::Option<sender::State>,
}
/// Nested message and enum types in `Sender`.
pub mod sender {
    #[derive(serde::Serialize)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Source {
        #[prost(string, tag = "1")]
        pub id: ::prost::alloc::string::String,
        #[prost(bool, tag = "2")]
        pub screen: bool,
        #[prost(string, optional, tag = "3")]
        pub metadata: ::core::option::Option<::prost::alloc::string::String>,
    }
    #[derive(serde::Serialize)]
    #[derive(Clone, Copy, PartialEq, ::prost::Message)]
    pub struct Config {
        #[prost(uint32, tag = "1")]
        pub priority: u32,
        #[prost(enumeration = "super::BitrateControlMode", tag = "2")]
        pub bitrate: i32,
    }
    #[derive(serde::Serialize)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct State {
        #[prost(message, optional, tag = "1")]
        pub config: ::core::option::Option<Config>,
        #[prost(message, optional, tag = "2")]
        pub source: ::core::option::Option<Source>,
    }
    #[derive(serde::Serialize)]
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Status {
        Active = 0,
        Inactive = 1,
    }
    impl Status {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Self::Active => "ACTIVE",
                Self::Inactive => "INACTIVE",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "ACTIVE" => Some(Self::Active),
                "INACTIVE" => Some(Self::Inactive),
                _ => None,
            }
        }
    }
}
#[derive(serde::Serialize)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Tracks {
    #[prost(message, repeated, tag = "1")]
    pub receivers: ::prost::alloc::vec::Vec<Receiver>,
    #[prost(message, repeated, tag = "2")]
    pub senders: ::prost::alloc::vec::Vec<Sender>,
}
#[derive(serde::Serialize)]
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct RoomInfoPublish {
    #[prost(bool, tag = "1")]
    pub peer: bool,
    #[prost(bool, tag = "2")]
    pub tracks: bool,
}
#[derive(serde::Serialize)]
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct RoomInfoSubscribe {
    #[prost(bool, tag = "1")]
    pub peers: bool,
    #[prost(bool, tag = "2")]
    pub tracks: bool,
}
#[derive(serde::Serialize)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AppContext {
    #[prost(string, optional, tag = "1")]
    pub app: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(serde::Serialize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Kind {
    Audio = 0,
    Video = 1,
}
impl Kind {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Audio => "AUDIO",
            Self::Video => "VIDEO",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "AUDIO" => Some(Self::Audio),
            "VIDEO" => Some(Self::Video),
            _ => None,
        }
    }
}
#[derive(serde::Serialize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum BitrateControlMode {
    DynamicConsumers = 0,
    MaxBitrate = 1,
}
impl BitrateControlMode {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::DynamicConsumers => "DYNAMIC_CONSUMERS",
            Self::MaxBitrate => "MAX_BITRATE",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "DYNAMIC_CONSUMERS" => Some(Self::DynamicConsumers),
            "MAX_BITRATE" => Some(Self::MaxBitrate),
            _ => None,
        }
    }
}
