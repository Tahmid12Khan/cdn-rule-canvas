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

/// Decompress a gzip body, bounding the INFLATED output to `max` bytes. Reads one
/// byte past the cap to detect overflow (decompression bomb); an overflow is
/// reported as `Err` so the caller falls back to pass-through.
pub fn gunzip(bytes: &[u8], max: usize) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    // Take `max + 1` so a body that exactly fills the cap still succeeds, while
    // anything larger is detected without buffering the whole inflated stream.
    let limit = (max as u64).saturating_add(1);
    GzDecoder::new(bytes).take(limit).read_to_end(&mut out)?;
    if out.len() > max {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "decompressed body exceeds cap",
        ));
    }
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
    max_decompressed: usize,
) -> Result<String, EncodingPassThrough> {
    if !is_modifiable(content_encoding) {
        return Err(EncodingPassThrough);
    }
    let is_gzip = matches!(
        content_encoding.map(|s| s.trim().to_ascii_lowercase()),
        Some(ref e) if e == "gzip"
    );
    let raw = if is_gzip {
        // An inflated body past the cap -> pass-through (serve original bytes).
        gunzip(&body, max_decompressed).map_err(|_| EncodingPassThrough)?
    } else {
        body.to_vec()
    };
    String::from_utf8(raw).map_err(|_| EncodingPassThrough)
}

/// Re-encode the (possibly modified) HTML back into the original encoding.
/// gzip -> gzip; everything else -> identity bytes. Returns `(bytes, gzipped)`
/// where `gzipped` reflects what was ACTUALLY produced: a gzip encode failure
/// yields identity bytes with `gzipped = false`, so the caller can keep the
/// `content-encoding` header in sync with the body rather than labelling
/// identity bytes as gzip.
pub fn reencode(content_encoding: Option<&str>, html: String) -> (Bytes, bool) {
    let is_gzip = matches!(
        content_encoding.map(|s| s.trim().to_ascii_lowercase()),
        Some(ref e) if e == "gzip"
    );
    if is_gzip {
        match gzip(html.as_bytes()) {
            Ok(b) => (Bytes::from(b), true),
            Err(_) => (Bytes::from(html.into_bytes()), false),
        }
    } else {
        (Bytes::from(html.into_bytes()), false)
    }
}
