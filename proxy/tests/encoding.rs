//! Unit tests for `infra::encoding`: the gzip round-trip, the decompression-bomb
//! bound (M1), and the re-encode/header sync flag (L-P2).

use bytes::Bytes;
use rre_proxy::infra::encoding::{decode_for_modify, gunzip, gzip, reencode};

#[test]
fn gunzip_within_cap_succeeds() {
    let plain = b"hello world".repeat(100);
    let compressed = gzip(&plain).unwrap();
    let out = gunzip(&compressed, plain.len()).unwrap();
    assert_eq!(out, plain);
}

#[test]
fn gunzip_past_cap_errors_decompression_bomb() {
    // A highly compressible body inflates far past a tiny cap.
    let plain = vec![0u8; 1_000_000];
    let compressed = gzip(&plain).unwrap();
    assert!(compressed.len() < plain.len());
    // Cap below the inflated size -> error (M1), so the caller passes through.
    assert!(gunzip(&compressed, 1024).is_err());
}

#[test]
fn decode_for_modify_gzip_bomb_passes_through() {
    let plain = vec![b'a'; 500_000];
    let compressed = gzip(&plain).unwrap();
    let body = Bytes::from(compressed);
    // Over-cap gzip body -> EncodingPassThrough (M1).
    assert!(decode_for_modify(Some("gzip"), body.clone(), 1024).is_err());
    // Under a generous cap it decodes fine.
    let decoded = decode_for_modify(Some("gzip"), body, 1_000_000).unwrap();
    assert_eq!(decoded.len(), plain.len());
}

#[test]
fn reencode_gzip_reports_gzipped_true() {
    let (bytes, gzipped) = reencode(Some("gzip"), "hello".to_string());
    assert!(gzipped, "gzip re-encode succeeded -> still gzip-labelled");
    // Round-trips back to the original.
    let back = gunzip(&bytes, 1024).unwrap();
    assert_eq!(back, b"hello");
}

#[test]
fn reencode_identity_reports_gzipped_false() {
    let (bytes, gzipped) = reencode(None, "hello".to_string());
    assert!(!gzipped, "identity output must not be labelled gzip (L-P2)");
    assert_eq!(&bytes[..], b"hello");
}
