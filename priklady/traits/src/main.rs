// Kapitola 5: Traits — spustiteľné príklady

trait Serialize {
    fn serialize(&self, buf: &mut Vec<u8>);
    fn wire_size(&self) -> usize;
}

struct U32Le(u32);
struct U16Be(u16);

impl Serialize for U32Le {
    fn serialize(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.0.to_le_bytes());
    }
    fn wire_size(&self) -> usize { 4 }
}

impl Serialize for U16Be {
    fn serialize(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.0.to_be_bytes());
    }
    fn wire_size(&self) -> usize { 2 }
}

fn encode_fields(fields: &[&dyn Serialize]) -> Vec<u8> {
    let total: usize = fields.iter().map(|f| f.wire_size()).sum();
    let mut buf = Vec::with_capacity(total);
    for f in fields {
        f.serialize(&mut buf);
    }
    buf
}

fn main() {
    let packet = encode_fields(&[
        &U16Be(0x0800),   // EtherType: IPv4
        &U32Le(0xDEAD_BEEF),
    ]);
    println!("packet bytes: {:02X?}", packet);
}
