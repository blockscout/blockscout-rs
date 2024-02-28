pub trait ToHex {
    fn to_hex(&self) -> String;
}

impl<T: AsRef<[u8]>> ToHex for T {
    fn to_hex(&self) -> String {
        format!("0x{}", hex::encode(self))
    }
}
