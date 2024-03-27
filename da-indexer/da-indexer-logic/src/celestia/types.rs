pub struct Blob {
    pub height: u64,
    pub namespace: Vec<u8>,
    pub commitment: Vec<u8>,
    pub data: Vec<u8>,
}
