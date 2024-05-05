pub mod endpoint;
pub mod media;
pub mod transport;

pub mod protobuf {
    pub mod shared {
        include!(concat!(env!("OUT_DIR"), "/shared.rs"));
    }

    pub mod features {
        include!(concat!(env!("OUT_DIR"), "/features.rs"));
    }

    pub mod conn {
        include!(concat!(env!("OUT_DIR"), "/conn.rs"));
    }

    pub mod gateway {
        include!(concat!(env!("OUT_DIR"), "/gateway.rs"));
    }
}
