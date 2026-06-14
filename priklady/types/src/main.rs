// Kapitola 3: Typy, Štruktúry, Enums — spustiteľné príklady

#[derive(Debug)]
#[allow(dead_code)]
struct IpHeader {
    version: u8,
    ihl: u8,
    tos: u8,
    total_len: u16,
    src: [u8; 4],
    dst: [u8; 4],
}

#[derive(Debug)]
enum ParseError {
    TooShort,
    InvalidVersion(u8),
}

fn parse_ip_header(buf: &[u8]) -> Result<IpHeader, ParseError> {
    if buf.len() < 20 {
        return Err(ParseError::TooShort);
    }
    let version = buf[0] >> 4;
    if version != 4 {
        return Err(ParseError::InvalidVersion(version));
    }
    Ok(IpHeader {
        version,
        ihl: buf[0] & 0x0F,
        tos: buf[1],
        total_len: u16::from_be_bytes([buf[2], buf[3]]),
        src: buf[12..16].try_into().unwrap(),
        dst: buf[16..20].try_into().unwrap(),
    })
}

fn main() {
    let packet = [
        0x45u8, 0x00, 0x00, 0x3c,
        0x00, 0x00, 0x40, 0x00,
        0x40, 0x06, 0x00, 0x00,
        192, 168, 1, 1,
        192, 168, 1, 2,
    ];

    match parse_ip_header(&packet) {
        Ok(hdr) => println!("Hlavička: {:?}", hdr),
        Err(ParseError::TooShort) => eprintln!("packet príliš krátky"),
        Err(ParseError::InvalidVersion(v)) => eprintln!("neznáma IP verzia: {}", v),
    }
}
