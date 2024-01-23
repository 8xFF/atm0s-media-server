use std::net::IpAddr;

use maxminddb::Reader;
use media_utils::F32;

pub struct Ip2Location {
    city_reader: Reader<Vec<u8>>,
}

impl Ip2Location {
    pub fn new(database_city: &str) -> Self {
        let city_reader = maxminddb::Reader::open_readfile(database_city).expect("Should open geoip database");
        Self { city_reader }
    }

    pub fn get_location(&self, ip: &IpAddr) -> Option<(F32<2>, F32<2>)> {
        match self.city_reader.lookup::<maxminddb::geoip2::City>(*ip) {
            Ok(res) => {
                let location = res.location?;
                match (location.latitude, location.longitude) {
                    (Some(lat), Some(lon)) => Some((F32::new(lat as f32), F32::new(lon as f32))),
                    _ => None,
                }
            }
            Err(err) => {
                log::error!("cannot get location of ip {} {}", ip, err);
                None
            }
        }
    }
}
