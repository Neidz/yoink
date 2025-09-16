pub fn url_encode(val: &str) -> String {
    val.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

const BASE64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(((input.len() + 2) / 3) * 4);

    let mut i = 0;
    while i + 3 <= input.len() {
        let b0 = input[i] as u32;
        let b1 = input[i + 1] as u32;
        let b2 = input[i + 2] as u32;
        let v = (b0 << 16) | (b1 << 8) | b2;

        out.push(BASE64_TABLE[((v >> 18) & 0x3F) as usize] as char);
        out.push(BASE64_TABLE[((v >> 12) & 0x3F) as usize] as char);
        out.push(BASE64_TABLE[((v >> 6) & 0x3F) as usize] as char);
        out.push(BASE64_TABLE[(v & 0x3F) as usize] as char);

        i += 3;
    }

    match input.len() - i {
        0 => {}
        1 => {
            let b0 = input[i] as u32;
            let v = b0 << 16;
            out.push(BASE64_TABLE[((v >> 18) & 0x3F) as usize] as char);
            out.push(BASE64_TABLE[((v >> 12) & 0x3F) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let b0 = input[i] as u32;
            let b1 = input[i + 1] as u32;
            let v = (b0 << 16) | (b1 << 8);
            out.push(BASE64_TABLE[((v >> 18) & 0x3F) as usize] as char);
            out.push(BASE64_TABLE[((v >> 12) & 0x3F) as usize] as char);
            out.push(BASE64_TABLE[((v >> 6) & 0x3F) as usize] as char);
            out.push('=');
        }
        _ => unreachable!(),
    }

    out
}

#[allow(unused)]
#[derive(Debug, PartialEq)]
pub enum DecodeError {
    InvalidLength,
    InvalidCharacter { ch: char, index: usize },
    InvalidPadding,
}

pub fn base64_decode(input: &str) -> Result<Vec<u8>, DecodeError> {
    let clean: Vec<char> = input.chars().filter(|c| !c.is_ascii_whitespace()).collect();

    if clean.is_empty() {
        return Ok(Vec::new());
    }

    if clean.len() % 4 != 0 {
        return Err(DecodeError::InvalidLength);
    }

    let pad = clean.iter().rev().take_while(|&&c| c == '=').count();
    if pad > 2 {
        return Err(DecodeError::InvalidPadding);
    }
    if clean[..clean.len() - pad].iter().any(|&c| c == '=') {
        return Err(DecodeError::InvalidPadding);
    }

    let mut out = Vec::with_capacity(clean.len() / 4 * 3 - pad);

    for (chunk_idx, chunk) in clean.chunks(4).enumerate() {
        let mut vals = [0u8; 4];
        for j in 0..4 {
            let ch = chunk[j];
            if ch == '=' {
                vals[j] = 0;
            } else {
                match val_from_base64_char(ch) {
                    Some(v) => vals[j] = v,
                    None => {
                        return Err(DecodeError::InvalidCharacter {
                            ch,
                            index: chunk_idx * 4 + j,
                        });
                    }
                }
            }
        }

        let combined = ((vals[0] as u32) << 18)
            | ((vals[1] as u32) << 12)
            | ((vals[2] as u32) << 6)
            | (vals[3] as u32);

        out.push(((combined >> 16) & 0xFF) as u8);

        if chunk[2] != '=' {
            out.push(((combined >> 8) & 0xFF) as u8);
        }

        if chunk[3] != '=' {
            out.push((combined & 0xFF) as u8);
        }
    }

    Ok(out)
}

fn val_from_base64_char(c: char) -> Option<u8> {
    match c {
        'A'..='Z' => Some((c as u8) - b'A'),
        'a'..='z' => Some((c as u8) - b'a' + 26),
        '0'..='9' => Some((c as u8) - b'0' + 52),
        '+' => Some(62),
        '/' => Some(63),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_examples() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"Glados"), "R2xhZG9z");
        assert_eq!(base64_encode(b"Chell"), "Q2hlbGw=");
        assert_eq!(
            base64_encode(b"The cake is a lie."),
            "VGhlIGNha2UgaXMgYSBsaWUu"
        );
    }

    #[test]
    fn round_trip_examples() {
        let cases: [&[u8]; 4] = [b"", b"Glados", b"Chell", b"The cake is a lie."];
        for &c in &cases {
            let enc = base64_encode(c);
            let dec = base64_decode(&enc).unwrap();
            assert_eq!(dec, c);
        }
    }
}
