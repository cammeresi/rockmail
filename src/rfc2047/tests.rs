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
fn decode_missing_question_after_enc() {
    // encoding letter not followed by '?'
    let input = b"=?UTF-8?Bxdata?=";
    assert_eq!(decode(input), "=?UTF-8?Bxdata?=");
}

#[test]
fn decode_invalid_utf8_lossy() {
    // Invalid UTF-8 bytes with =? to enter the main decode loop
    let input = b"\xff=?bogus";
    assert_eq!(decode(input), "\u{FFFD}=?bogus");
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
fn utf8_floor_first_char_wider_than_at() {
    // 'é' is 2 bytes; at=1 rounds down to 0, triggering the i==0 branch
    assert_eq!(utf8_floor("é", 1), 2);
}

#[test]
fn enc_try_from_invalid() {
    assert_eq!(Enc::try_from(b'X'), Err(()));
}

#[test]
fn q_decode_invalid_hex_pair() {
    assert_eq!(q_decode(b"=ZZ"), b"=ZZ");
}

#[test]
fn q_roundtrip() {
    let data = b"caf\xc3\xa9 l\xc3\xa0";
    let encoded = q_encode(data);
    let decoded = q_decode(encoded.as_bytes());
    assert_eq!(decoded, data);
}

#[test]
fn hex_val_digits() {
    for b in b'0'..=b'9' {
        assert_eq!(hex_val(b), Some(b - b'0'));
    }
}

#[test]
fn hex_val_upper() {
    for (i, b) in (b'A'..=b'F').enumerate() {
        assert_eq!(hex_val(b), Some(i as u8 + 10));
    }
}

#[test]
fn hex_val_lower() {
    for (i, b) in (b'a'..=b'f').enumerate() {
        assert_eq!(hex_val(b), Some(i as u8 + 10));
    }
}

#[test]
fn hex_val_invalid() {
    assert_eq!(hex_val(b'g'), None);
    assert_eq!(hex_val(b'G'), None);
    assert_eq!(hex_val(b'/'), None);
    assert_eq!(hex_val(b':'), None);
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
