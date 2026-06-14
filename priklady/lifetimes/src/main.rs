// Kapitola 6: Lifetimes — zero-copy parser príklad

struct Frame<'a> {
    header: &'a [u8],
    payload: &'a [u8],
}

fn parse_frame(buf: &[u8]) -> Option<Frame<'_>> {
    if buf.len() < 4 {
        return None;
    }
    let payload_len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
    if buf.len() < 4 + payload_len {
        return None;
    }
    Some(Frame {
        header: &buf[..4],
        payload: &buf[4..4 + payload_len],
    })
}

fn main() {
    let raw = [0x01u8, 0x02, 0x00, 0x05, b'H', b'e', b'l', b'l', b'o'];
    if let Some(frame) = parse_frame(&raw) {
        println!("header: {:02X?}", frame.header);
        println!("payload: {:?}", std::str::from_utf8(frame.payload).unwrap());
    }
}
