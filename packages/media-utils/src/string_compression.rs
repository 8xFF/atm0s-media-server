use flate2::write::ZlibEncoder;
use flate2::Compression;
use lz4_flex::compress_prepend_size;
use std::io::{Read, Write};

pub struct StringCompression {}

impl Default for StringCompression {
    fn default() -> Self {
        Self {}
    }
}

impl StringCompression {
    pub fn compress(&self, data: &str) -> Vec<u8> {
        compress_prepend_size(data.as_bytes())
    }

    pub fn compress_bytes(&self, data: &[u8]) -> Vec<u8> {
        compress_prepend_size(data)
    }

    pub fn uncompress(&self, data: &[u8]) -> Option<String> {
        let uncompressed = lz4_flex::decompress_size_prepended(data).ok()?;
        String::from_utf8(uncompressed).ok()
    }

    pub fn compress_zlib(&self, data: &str) -> Vec<u8> {
        //TODO dont use unwrap
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(data.as_bytes()).expect("Should work");
        e.finish().expect("Should work")
    }

    pub fn compress_zlib_bytes(&self, data: &[u8]) -> Vec<u8> {
        //TODO dont use unwrap
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(data).expect("Should work");
        e.finish().expect("Should work")
    }

    pub fn uncompress_zlib(&self, data: &[u8]) -> Option<String> {
        let mut d = flate2::read::ZlibDecoder::new(data);
        let mut s = String::new();
        d.read_to_string(&mut s).ok()?;
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    //test compress and uncompress
    use super::*;

    #[test]
    fn test_compress() {
        let data = "hello world";
        let compression = StringCompression::default();
        let compressed = compression.compress(data);
        let uncompress = compression.uncompress(&compressed);
        assert_eq!(uncompress, Some(data.to_string()));
    }

    #[test]
    fn test_compress_zlib() {
        let compressed = vec![120, 156, 203, 72, 205, 201, 201, 87, 40, 207, 47, 202, 73, 1, 0, 26, 11, 4, 93];
        let compression = StringCompression::default();
        let uncompress = compression.uncompress_zlib(&compressed);
        assert_eq!(uncompress, Some("hello world".to_string()));

        let compressed = compression.compress_zlib("test1");
        let uncompress = compression.uncompress_zlib(&compressed);
        assert_eq!(uncompress, Some("test1".to_string()));
    }
}
