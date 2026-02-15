use proptest::prelude::*;

use super::*;

#[test]
fn decode_plain_ascii() {
    assert_eq!(decode(b"hello world"), "hello world");
}

#[test]
fn decode_base64_utf8() {
    let input = b"=?UTF-8?B?Y2Fmw6k=?=";
    assert_eq!(decode(input), "café");
}

#[test]
fn decode_qp_utf8() {
    let input = b"=?UTF-8?Q?caf=C3=A9?=";
    assert_eq!(decode(input), "café");
}

#[test]
fn decode_qp_underscore_is_space() {
    let input = b"=?UTF-8?Q?hello_world?=";
    assert_eq!(decode(input), "hello world");
}

#[test]
fn decode_iso8859() {
    let input = b"=?ISO-8859-1?Q?caf=E9?=";
    assert_eq!(decode(input), "café");
}

#[test]
fn decode_adjacent_collapse_whitespace() {
    // Two adjacent encoded words separated by whitespace should have
    // the whitespace collapsed per RFC 2047 §6.2
    let input = b"=?UTF-8?B?Y2Fm?= =?UTF-8?B?w6k=?=";
    assert_eq!(decode(input), "café");
}

#[test]
fn decode_mixed_plain_and_encoded() {
    let input = b"Re: =?UTF-8?B?Y2Fmw6k=?= test";
    assert_eq!(decode(input), "Re: café test");
}

#[test]
fn decode_invalid_passthrough() {
    let input = b"=?BOGUS";
    assert_eq!(decode(input), "=?BOGUS");
}

#[test]
fn decode_case_insensitive_encoding() {
    let input = b"=?utf-8?b?Y2Fmw6k=?=";
    assert_eq!(decode(input), "café");
}

#[test]
fn encode_ascii_passthrough() {
    assert_eq!(encode("hello", Enc::B), "hello");
    assert_eq!(encode("hello", Enc::Q), "hello");
}

#[test]
fn encode_b_roundtrip() {
    let text = "café";
    let encoded = encode(text, Enc::B);
    assert!(encoded.starts_with("=?UTF-8?B?"));
    assert!(encoded.ends_with("?="));
    assert_eq!(decode(encoded.as_bytes()), text);
}

#[test]
fn encode_q_roundtrip() {
    let text = "café";
    let encoded = encode(text, Enc::Q);
    assert!(encoded.starts_with("=?UTF-8?Q?"));
    assert!(encoded.ends_with("?="));
    assert_eq!(decode(encoded.as_bytes()), text);
}

#[test]
fn enc_detect_b() {
    let input = b"=?UTF-8?B?Y2Fmw6k=?=";
    assert_eq!(Enc::detect(input), Some(Enc::B));
}

#[test]
fn enc_detect_q() {
    let input = b"=?UTF-8?Q?caf=C3=A9?=";
    assert_eq!(Enc::detect(input), Some(Enc::Q));
}

#[test]
fn enc_detect_none() {
    assert_eq!(Enc::detect(b"plain text"), None);
}

#[test]
fn q_roundtrip() {
    let data = b"caf\xc3\xa9 l\xc3\xa0";
    let encoded = q_encode(data);
    let decoded = q_decode(encoded.as_bytes());
    assert_eq!(decoded, data);
}

proptest! {
    #[test]
    fn encode_b_roundtrip_prop(s in "\\PC{1,200}") {
        let encoded = encode(&s, Enc::B);
        let decoded = decode(encoded.as_bytes());
        prop_assert_eq!(decoded.as_ref(), s.as_str());
    }

    #[test]
    fn encode_q_roundtrip_prop(s in "\\PC{1,200}") {
        let encoded = encode(&s, Enc::Q);
        let decoded = decode(encoded.as_bytes());
        prop_assert_eq!(decoded.as_ref(), s.as_str());
    }
}
