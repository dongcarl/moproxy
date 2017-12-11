use std::str::from_utf8;
use std::ops::Range;

pub struct TlsClientHello<'a> {
    pub server_name: Option<&'a str>,
}

struct TlsRecord<'a> {
    content_type: &'a u8,
    version_major: &'a u8,
    #[allow(dead_code)]
    version_minor: &'a u8,
    fragment: &'a [u8],
}

fn truncate(data: &[u8], len_pos: Range<usize>)
        -> Result<&[u8], &'static str> {
    let len_bits = data.get(len_pos.clone())
        .ok_or("lack data to decode length")?;
    let mut len = 0usize;
    for bit in len_bits {
        len = len << 8 | (*bit as usize);
    }
    return data.get(len_pos.end..len_pos.end + len)
        .ok_or("not enough data");
}

fn drop_before(data: &[u8], len_pos: Range<usize>)
        -> Result<&[u8], &'static str> {
    let len = truncate(data, len_pos.clone())?.len();
    return Ok(&data[len_pos.end + len..]);
}

fn parse_tls_record<'a>(data: &'a [u8])
        -> Result<TlsRecord<'a>, &'static str> {
    let fragment = truncate(data, 3..5)?;
    Ok(TlsRecord {
        content_type: &data[0],
        version_major: &data[1],
        version_minor: &data[2],
        fragment,
    })
}

pub fn parse_client_hello<'a>(data: &'a [u8])
        -> Result<TlsClientHello<'a>, &'static str> {
    let TlsRecord {
        content_type: &ctype,
        version_major: &version,
        fragment,
        ..
    } = parse_tls_record(data)?;
    if version != 3 {
        return Err("unknown tls version");
    }
    if ctype != 22 {
        return Err("not handshake");
    }

    // 0: handshake type
    if fragment.get(0) != Some(&1) {
        return Err("not client hello");
    }
    let hello = truncate(fragment, 1..4)?;
    // 0..2: client version
    if hello.get(0) != Some(&3) {
        return Err("unsupported client version");
    }
    // 2..34: 32-bytes random, dropped
    // 34+: session id, dropped
    let remaining = drop_before(hello, 34..35)?;
    // cipher suite, dropped
    let remaining = drop_before(remaining, 0..2)?;
    // compression methods, dropped
    let remaining = drop_before(remaining, 0..1)?;
    // 2-byte length of extensions
    let mut exts = truncate(remaining, 0..2)?;
    let mut server_name = None;
    while exts.len() >= 4 {
        // 0..2: extension type
        let ext_type = &exts[0..2];
        // 2..4: extension length
        let ext_data = truncate(exts, 2..4)?;
        exts = drop_before(exts, 2..4)?;
        if ext_type == &[0, 0] { // server name indication
            let mut data = truncate(ext_data, 0..2)?;
            while data.len() > 3 {
                let value = truncate(data, 1..3)?;
                let name_type = data[0];
                data = drop_before(data, 1..3)?;
                if name_type == 0 { // hostname
                    server_name = Some(parse_server_name(value)?);
                }
            }
        }
    }

    Ok(TlsClientHello {
        server_name: server_name,
    })
}

fn parse_server_name(value: &[u8]) -> Result<&str, &'static str> {
    let name = match from_utf8(value) {
        Ok(s) => s,
        Err(_) => return Err("server name not utf-8 string"),
    };
    if name.as_bytes().len() > 255 {
        return Err("server name too long");
    }
    if !name.chars().all(|c| c.is_digit(36) || c == '.' || c == '-'
                         || c == '_') {
        return Err("illegal char in server name");
    }
    Ok(name)
}


#[test]
fn test_parse_without_server_name() {
    let data = [
        0x16, 0x03, 0x01, 0x00, 0xa1, 0x01, 0x00, 0x00,
        0x9d, 0x03, 0x03, 0x52, 0x36, 0x2c, 0x10, 0x12,
        0xcf, 0x23, 0x62, 0x82, 0x56, 0xe7, 0x45, 0xe9,
        0x03, 0xce, 0xa6, 0x96, 0xe9, 0xf6, 0x2a, 0x60,
        0xba, 0x0a, 0xe8, 0x31, 0x1d, 0x70, 0xde, 0xa5,
        0xe4, 0x19, 0x49, 0x00, 0x00, 0x04, 0xc0, 0x30,
        0x00, 0xff, 0x02, 0x01, 0x00, 0x00, 0x6f, 0x00,
        0x0b, 0x00, 0x04, 0x03, 0x00, 0x01, 0x02, 0x00,
        0x0a, 0x00, 0x34, 0x00, 0x32, 0x00, 0x0e, 0x00,
        0x0d, 0x00, 0x19, 0x00, 0x0b, 0x00, 0x0c, 0x00,
        0x18, 0x00, 0x09, 0x00, 0x0a, 0x00, 0x16, 0x00,
        0x17, 0x00, 0x08, 0x00, 0x06, 0x00, 0x07, 0x00,
        0x14, 0x00, 0x15, 0x00, 0x04, 0x00, 0x05, 0x00,
        0x12, 0x00, 0x13, 0x00, 0x01, 0x00, 0x02, 0x00,
        0x03, 0x00, 0x0f, 0x00, 0x10, 0x00, 0x11, 0x00,
        0x23, 0x00, 0x00, 0x00, 0x0d, 0x00, 0x22, 0x00,
        0x20, 0x06, 0x01, 0x06, 0x02, 0x06, 0x03, 0x05,
        0x01, 0x05, 0x02, 0x05, 0x03, 0x04, 0x01, 0x04,
        0x02, 0x04, 0x03, 0x03, 0x01, 0x03, 0x02, 0x03,
        0x03, 0x02, 0x01, 0x02, 0x02, 0x02, 0x03, 0x01,
        0x01, 0x00, 0x0f, 0x00, 0x01, 0x01,
    ];
    if let Ok(TlsRecord {
        content_type: &content_type,
        version_major: &version_major,
        version_minor: &version_minor,
        fragment,
    }) = parse_tls_record(&data) {
        assert_eq!(22, content_type);
        assert_eq!(3, version_major);
        assert_eq!(1, version_minor);
        assert_eq!(161, fragment.len());
        assert_eq!(1, fragment[0]);
        assert_eq!(Some(&1), fragment.last());
    } else {
        assert!(false);
    };

    let TlsClientHello { server_name, .. } = parse_client_hello(&data)
        .unwrap();
    assert_eq!(None, server_name);
}

#[test]
fn test_parse_with_server_name() {
    let data = [
        0x16, 0x03, 0x01, 0x00, 0xba, 0x01, 0x00, 0x00,
        0xb6, 0x03, 0x03, 0xce, 0xf3, 0xc8, 0x77, 0x36,
        0x6a, 0x81, 0x3b, 0x2f, 0x22, 0xc8, 0xd3, 0x29,
        0xed, 0xf8, 0xb6, 0xec, 0xd9, 0x73, 0xfb, 0x76,
        0x66, 0x6c, 0xbb, 0xa0, 0x50, 0xbd, 0x42, 0x13,
        0xd5, 0xc4, 0xf1, 0x00, 0x00, 0x1e, 0xc0, 0x2b,
        0xc0, 0x2f, 0xcc, 0xa9, 0xcc, 0xa8, 0xc0, 0x2c,
        0xc0, 0x30, 0xc0, 0x0a, 0xc0, 0x09, 0xc0, 0x13,
        0xc0, 0x14, 0x00, 0x33, 0x00, 0x39, 0x00, 0x2f,
        0x00, 0x35, 0x00, 0x0a, 0x01, 0x00, 0x00, 0x6f,
        0x00, 0x00, 0x00, 0x13, 0x00, 0x11, 0x00, 0x00,
        0x0e, 0x77, 0x77, 0x77, 0x2e, 0x67, 0x6f, 0x6f,
        0x67, 0x6c, 0x65, 0x2e, 0x63, 0x6f, 0x6d, 0x00,
        0x17, 0x00, 0x00, 0xff, 0x01, 0x00, 0x01, 0x00,
        0x00, 0x0a, 0x00, 0x0a, 0x00, 0x08, 0x00, 0x1d,
        0x00, 0x17, 0x00, 0x18, 0x00, 0x19, 0x00, 0x0b,
        0x00, 0x02, 0x01, 0x00, 0x00, 0x23, 0x00, 0x00,
        0x00, 0x10, 0x00, 0x0e, 0x00, 0x0c, 0x02, 0x68,
        0x32, 0x08, 0x68, 0x74, 0x74, 0x70, 0x2f, 0x31,
        0x2e, 0x31, 0x00, 0x05, 0x00, 0x05, 0x01, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x0d, 0x00, 0x18, 0x00,
        0x16, 0x04, 0x03, 0x05, 0x03, 0x06, 0x03, 0x08,
        0x04, 0x08, 0x05, 0x08, 0x06, 0x04, 0x01, 0x05,
        0x01, 0x06, 0x01, 0x02, 0x03, 0x02, 0x01,
    ];
    let TlsClientHello { server_name, .. } = parse_client_hello(&data)
        .unwrap();
    assert_eq!(Some("www.google.com"), server_name);
}

