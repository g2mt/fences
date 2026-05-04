use serde_json;

use super::*;

#[test]
fn test_rgb_serialize_deserialize() {
    // #RRGGBB => alpha defaults to 0xFF
    let json = "\"#AABBCC\"";
    let color: Color = serde_json::from_str(json).unwrap();
    assert_eq!(color.r(), 0xAA);
    assert_eq!(color.g(), 0xBB);
    assert_eq!(color.b(), 0xCC);
    assert_eq!(color.a(), 0xFF);

    let serialized = serde_json::to_string(&color).unwrap();
    assert_eq!(serialized, "\"#FFAABBCC\"");
}

#[test]
fn test_argb_serialize_deserialize() {
    // #AARRGGBB is preserved
    let json = "\"#80AABBCC\"";
    let color: Color = serde_json::from_str(json).unwrap();
    assert_eq!(color.r(), 0xAA);
    assert_eq!(color.g(), 0xBB);
    assert_eq!(color.b(), 0xCC);
    assert_eq!(color.a(), 0x80);

    let serialized = serde_json::to_string(&color).unwrap();
    assert_eq!(serialized, json);
}

#[test]
fn test_argb_roundtrip_with_opaque() {
    // six-digit string -> fully opaque alpha when ACCEPTS_ALPHA would be true
    let json = "\"#336699\"";
    let color: Color = serde_json::from_str(json).unwrap();
    assert_eq!(color.r(), 0x33);
    assert_eq!(color.g(), 0x66);
    assert_eq!(color.b(), 0x99);
    assert_eq!(color.a(), 0xFF);

    let serialized = serde_json::to_string(&color).unwrap();
    assert_eq!(serialized, "\"#FF336699\"");
}

#[test]
fn test_accepts_eight_digits() {
    // eight‑digit strings are accepted
    let json = "\"#80AABBCC\"";
    let color: Color = serde_json::from_str(json).unwrap();
    assert_eq!(color.a(), 0x80);
    assert_eq!(color.r(), 0xAA);
    assert_eq!(color.g(), 0xBB);
    assert_eq!(color.b(), 0xCC);
}

#[test]
fn test_rgb_getters_from_u32() {
    // internal representation ARGB: A=0xFF, R=0x11, G=0x22, B=0x33
    let color = Color(0xFF112233);
    assert_eq!(color.r(), 0x11);
    assert_eq!(color.g(), 0x22);
    assert_eq!(color.b(), 0x33);
    assert_eq!(color.a(), 0xFF);
}
