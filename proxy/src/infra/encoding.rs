//! Body encoding helpers. We only modify bodies we can fully decode: identity
//! and gzip. `br` / `deflate` are passed through unmodified (caller warns).

use std::io::{Read, Write};

use bytes::Bytes;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;

/// Signals the caller to pass the body through unchanged (encoding we cannot
/// safely round-trip for modification).
#[derive(Debug, thiserror::Error)]
#[error("unsupported content-encoding for modification")]
pub struct EncodingPassThrough;

pub fn gunzip(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    GzDecoder::new(bytes).read_to_end(&mut out)?;
    Ok(out)
}

pub fn gzip(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    encoder.finish()
}

/// True for an encoding we can decode for modification (identity or gzip).
fn is_modifiable(content_encoding: Option<&str>) -> bool {
    match content_encoding.map(|s| s.trim().to_ascii_lowercase()) {
        None => true,
        Some(e) if e.is_empty() || e == "identity" || e == "gzip" => true,
        _ => false,
    }
}

/// Decode a body into a UTF-8 string for modification. `br`/`deflate` -> Err so
/// the caller passes the original bytes through unchanged.
pub fn decode_for_modify(
    content_encoding: Option<&str>,
    body: Bytes,
) -> Result<String, EncodingPassThrough> {
    if !is_modifiable(content_encoding) {
        return Err(EncodingPassThrough);
    }
    let is_gzip = matches!(
        content_encoding.map(|s| s.trim().to_ascii_lowercase()),
        Some(ref e) if e == "gzip"
    );
    let raw = if is_gzip {
        gunzip(&body).map_err(|_| EncodingPassThrough)?
    } else {
        body.to_vec()
    };
    String::from_utf8(raw).map_err(|_| EncodingPassThrough)
}

/// Re-encode the (possibly modified) HTML back into the original encoding.
/// gzip -> gzip; everything else -> identity bytes.
pub fn reencode(content_encoding: Option<&str>, html: String) -> Bytes {
    let is_gzip = matches!(
        content_encoding.map(|s| s.trim().to_ascii_lowercase()),
        Some(ref e) if e == "gzip"
    );
    if is_gzip {
        match gzip(html.as_bytes()) {
            Ok(b) => Bytes::from(b),
            Err(_) => Bytes::from(html.into_bytes()),
        }
    } else {
        Bytes::from(html.into_bytes())
    }
}
