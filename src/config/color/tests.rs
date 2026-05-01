use serde_json;

use super::*;

#[test]
fn test_rgb_serialize_deserialize() {
    // Color<false> – #RRGGBB only
    let json = "\"#AABBCC\"";
    let color: Color<false> = serde_json::from_str(json).unwrap();
    assert_eq!(color.r(), 0xAA);
    assert_eq!(color.g(), 0xBB);
    assert_eq!(color.b(), 0xCC);
    assert_eq!(color.a(), 0); // alpha is ignored, stored as 0

    let serialized = serde_json::to_string(&color).unwrap();
    assert_eq!(serialized, json);
}

#[test]
fn test_argb_serialize_deserialize() {
    // Color<true> – #AARRGGBB allowed
    let json = "\"#80AABBCC\"";
    let color: Color<true> = serde_json::from_str(json).unwrap();
    assert_eq!(color.r(), 0xAA);
    assert_eq!(color.g(), 0xBB);
    assert_eq!(color.b(), 0xCC);
    assert_eq!(color.a(), 0x80);

    let serialized = serde_json::to_string(&color).unwrap();
    assert_eq!(serialized, json);
}

#[test]
fn test_argb_roundtrip_with_opaque() {
    // using 6‑digit string
    let json = "\"#336699\"";
    let color: Color<true> = serde_json::from_str(json).unwrap();
    assert_eq!(color.r(), 0x33);
    assert_eq!(color.g(), 0x66);
    assert_eq!(color.b(), 0x99);
    assert_eq!(color.a(), 0xFF); // implicit opaque

    let serialized = serde_json::to_string(&color).unwrap();
    // Should output alpha because ACCEPTS_ALPHA is true
    assert_eq!(serialized, "\"#FF336699\"");
}

#[test]
fn test_rgb_rejects_eight_digits() {
    let json = "\"#80AABBCC\"";
    let result: Result<Color<false>, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn test_rgb_getters_from_u32() {
    // Internal representation ARGB: A=0xFF, R=0x11, G=0x22, B=0x33
    // So u32 = 0xFF112233
    let color = Color::<false>(0xFF112233);
    assert_eq!(color.r(), 0x11);
    assert_eq!(color.g(), 0x22);
    assert_eq!(color.b(), 0x33);
    assert_eq!(color.a(), 0xFF);
}
