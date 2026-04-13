use std::io::Read;
use crate::error::IngestError;

/// Decompress request body based on Content-Encoding.
pub fn decompress_body(body: &[u8], encoding: Option<&str>) -> Result<Vec<u8>, IngestError> {
    match encoding {
        None | Some("identity") => Ok(body.to_vec()),
        Some("gzip") => {
            let mut decoder = flate2::read::GzDecoder::new(body);
            let mut decompressed = Vec::with_capacity(body.len() * 2);
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| IngestError::Decompression(format!("gzip: {e}")))?;
            Ok(decompressed)
        }
        Some("deflate") => {
            let mut decoder = flate2::read::ZlibDecoder::new(body);
            let mut decompressed = Vec::with_capacity(body.len() * 2);
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| IngestError::Decompression(format!("deflate: {e}")))?;
            Ok(decompressed)
        }
        Some("br") => {
            let mut decoder = brotli::Decompressor::new(body, 4096);
            let mut decompressed = Vec::with_capacity(body.len() * 2);
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| IngestError::Decompression(format!("brotli: {e}")))?;
            Ok(decompressed)
        }
        Some(enc) => Err(IngestError::Decompression(format!(
            "unsupported encoding: {enc}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_decompress_identity() {
        let data = b"hello world";
        let result = decompress_body(data, None).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_decompress_gzip() {
        let original = b"hello compressed world";
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        let result = decompress_body(&compressed, Some("gzip")).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_decompress_deflate() {
        let original = b"hello compressed world";
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        let result = decompress_body(&compressed, Some("deflate")).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_decompress_unsupported() {
        assert!(decompress_body(b"data", Some("zstd")).is_err());
    }

    #[test]
    fn test_decompress_brotli() {
        let original = b"hello brotli compressed world";
        let mut compressed = Vec::new();
        {
            let mut encoder = brotli::CompressorWriter::new(&mut compressed, 4096, 4, 22);
            encoder.write_all(original).unwrap();
        }

        let result = decompress_body(&compressed, Some("br")).unwrap();
        assert_eq!(result, original);
    }
}
