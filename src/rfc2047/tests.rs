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

#[test]
fn encode_b_exact() {
    assert_eq!(encode("café", Enc::B), "=?UTF-8?B?Y2Fmw6k=?=");
}

#[test]
fn encode_q_exact() {
    assert_eq!(encode("café", Enc::Q), "=?UTF-8?Q?caf=C3=A9?=");
}

#[test]
fn encode_b_mixed() {
    assert_eq!(
        encode("hello café world", Enc::B),
        "=?UTF-8?B?aGVsbG8gY2Fmw6kgd29ybGQ=?=",
    );
}

#[test]
fn encode_q_mixed() {
    assert_eq!(
        encode("hello café world", Enc::Q),
        "=?UTF-8?Q?hello_caf=C3=A9_world?=",
    );
}

#[test]
fn encode_q_at_limit() {
    // 57 'a' + 'é' fills exactly 75 chars: 10 prefix + 57 + 6 (=C3=A9) + 2 suffix
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaé";
    let encoded = encode(input, Enc::Q);
    assert_eq!(encoded.len(), MAX_WORD);
    assert!(!encoded.contains("\r\n"));
}

#[test]
fn encode_q_over_limit() {
    // 58 'a' + 'é': the é forces a fold
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaé";
    let encoded = encode(input, Enc::Q);
    let words: Vec<_> = encoded.split("\r\n ").collect();
    assert_eq!(words.len(), 2);
    assert_eq!(
        words[0],
        "=?UTF-8?Q?aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa?=",
    );
    assert_eq!(words[1], "=?UTF-8?Q?=C3=A9?=");
}

#[test]
fn encode_q_multi_word() {
    // 25 'é': 10 per word (60 Q chars each) + 5 in last word
    let input = "é".repeat(25);
    let encoded = encode(&input, Enc::Q);
    let words: Vec<_> = encoded.split("\r\n ").collect();
    assert_eq!(words.len(), 3);
    let e = "=C3=A9";
    assert_eq!(words[0], format!("=?UTF-8?Q?{0}?=", e.repeat(10)));
    assert_eq!(words[1], format!("=?UTF-8?Q?{0}?=", e.repeat(10)));
    assert_eq!(words[2], format!("=?UTF-8?Q?{0}?=", e.repeat(5)));
}

#[test]
fn encode_b_multi_word() {
    // 45 'é' = 90 bytes, split into 44+44+2 byte chunks
    let input = "é".repeat(45);
    let encoded = encode(&input, Enc::B);
    let words: Vec<_> = encoded.split("\r\n ").collect();
    assert_eq!(words.len(), 3);
    // 22 é's (44 bytes) each for the first two words
    let chunk = "é".repeat(22);
    let b64 = STANDARD.encode(chunk.as_bytes());
    assert_eq!(words[0], format!("=?UTF-8?B?{b64}?="));
    assert_eq!(words[1], format!("=?UTF-8?B?{b64}?="));
    // Last word: 1 é (2 bytes)
    assert_eq!(words[2], "=?UTF-8?B?w6k=?=");
}

#[test]
fn encode_emoji() {
    assert_eq!(encode("🎉", Enc::Q), "=?UTF-8?Q?=F0=9F=8E=89?=");
    assert_eq!(encode("🎉", Enc::B), "=?UTF-8?B?8J+OiQ==?=");
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
