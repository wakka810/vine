use serde_json::json;
use vine_core::{API_GUEST_WRITE_INLINE_SERIALIZATION_MAX_BYTES, ApiGuestWrite, GuestAddress};

#[test]
fn guest_write_transport_inlines_small_bytes_and_digests_large_bytes() {
    let inline = ApiGuestWrite {
        address: GuestAddress(0x2000_0000),
        bytes: vec![0x12, 0x34, 0x56],
    };
    assert_eq!(
        serde_json::to_value(&inline).unwrap(),
        json!({
            "address": 0x2000_0000,
            "bytes": [0x12, 0x34, 0x56],
        })
    );

    let large = ApiGuestWrite {
        address: GuestAddress(0x3000_0000),
        bytes: vec![0xa5; API_GUEST_WRITE_INLINE_SERIALIZATION_MAX_BYTES + 1],
    };
    let serialized = serde_json::to_value(&large).unwrap();
    assert_eq!(serialized["address"], 0x3000_0000);
    assert_eq!(
        serialized["byteLength"],
        API_GUEST_WRITE_INLINE_SERIALIZATION_MAX_BYTES + 1
    );
    assert_eq!(
        serialized["contentSha256"],
        "a3253bfa7d790dc6f5d41b75421bcc2c606e8043f9fad4331e8aae0a6c504cfa"
    );
    assert!(serialized.get("bytes").is_none());
    assert_eq!(
        large.bytes.len(),
        API_GUEST_WRITE_INLINE_SERIALIZATION_MAX_BYTES + 1
    );
}
