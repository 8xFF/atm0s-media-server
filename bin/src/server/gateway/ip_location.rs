use std::net::IpAddr;

use maxminddb::Reader;

pub struct Ip2Location {
    city_reader: Reader<Vec<u8>>,
}

impl Ip2Location {
    pub fn new(database_city: &str) -> Self {
        let city_reader = maxminddb::Reader::open_readfile(database_city).expect("Failed to open geoip database");
        Self { city_reader }
    }

    pub fn get_location(&self, ip: &IpAddr) -> Option<(f32, f32)> {
        match self.city_reader.lookup::<maxminddb::geoip2::City>(*ip) {
            Ok(res) => {
                let location = res.location?;
                match (location.latitude, location.longitude) {
                    (Some(lat), Some(lon)) => Some((lat as f32, lon as f32)),
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
