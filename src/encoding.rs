//! Encoding check for the JSON files rum-meter reads (state and prices).
//!
//! The files must be UTF-8. One leading UTF-8 BOM is skipped. UTF-16 and
//! UTF-32 are refused, with or without a BOM, and nothing is decoded. Without
//! a BOM they are recognised the way RFC 4627 describes: the first character
//! of a JSON text is ASCII, so the NUL bytes among the first 4 bytes show the
//! encoding. Valid UTF-8 JSON never starts with a NUL byte.

/// A wide encoding that is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wide {
    Utf16,
    Utf32,
}

impl Wide {
    pub fn name(self) -> &'static str {
        match self {
            Self::Utf16 => "UTF-16",
            Self::Utf32 => "UTF-32",
        }
    }
}

const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// The bytes to parse: without a leading UTF-8 BOM. UTF-32 is checked
/// before UTF-16 because the UTF-32LE BOM starts with the UTF-16LE BOM.
pub fn utf8_body(bytes: &[u8]) -> Result<&[u8], Wide> {
    match bytes {
        [0xFF, 0xFE, 0x00, 0x00, ..] | [0x00, 0x00, 0xFE, 0xFF, ..] => Err(Wide::Utf32),
        [0xFF, 0xFE, ..] | [0xFE, 0xFF, ..] => Err(Wide::Utf16),
        [0x00, 0x00, 0x00, b, ..] | [b, 0x00, 0x00, 0x00, ..] if *b != 0 => Err(Wide::Utf32),
        [0x00, b, ..] | [b, 0x00, ..] if *b != 0 => Err(Wide::Utf16),
        _ => Ok(bytes.strip_prefix(&UTF8_BOM[..]).unwrap_or(bytes)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(text: &str, utf32: bool, le: bool, bom: bool) -> Vec<u8> {
        let mut out = Vec::new();
        let chars = bom.then_some('\u{FEFF}').into_iter().chain(text.chars());
        for c in chars {
            if utf32 {
                let unit = u32::from(c);
                out.extend(if le {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            } else {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.extend(if le {
                        unit.to_le_bytes()
                    } else {
                        unit.to_be_bytes()
                    });
                }
            }
        }
        out
    }

    #[test]
    fn utf8_with_and_without_bom() {
        assert_eq!(utf8_body(b"{}"), Ok(&b"{}"[..]));
        assert_eq!(utf8_body(b"\xEF\xBB\xBF{}"), Ok(&b"{}"[..]));
        assert_eq!(utf8_body(b" \n{}"), Ok(&b" \n{}"[..]));
        assert_eq!(utf8_body(b""), Ok(&b""[..]));
        // Only one BOM is skipped; a second one is left for the JSON parser.
        assert_eq!(
            utf8_body(b"\xEF\xBB\xBF\xEF\xBB\xBF{}"),
            Ok(&b"\xEF\xBB\xBF{}"[..])
        );
    }

    #[test]
    fn wide_encodings_with_and_without_bom() {
        for text in ["{}", " {\"version\":4}", "\r\n{}"] {
            for le in [true, false] {
                for bom in [true, false] {
                    assert_eq!(utf8_body(&wide(text, false, le, bom)), Err(Wide::Utf16));
                    assert_eq!(utf8_body(&wide(text, true, le, bom)), Err(Wide::Utf32));
                }
            }
        }
    }

    #[test]
    fn short_inputs() {
        assert_eq!(utf8_body(b"{"), Ok(&b"{"[..]));
        assert_eq!(utf8_body(b"{\x00"), Err(Wide::Utf16));
        assert_eq!(utf8_body(b"\x00{"), Err(Wide::Utf16));
        // Only NUL bytes are not a wide encoding; the JSON parser refuses them.
        assert_eq!(utf8_body(b"\x00\x00"), Ok(&b"\x00\x00"[..]));
    }
}
