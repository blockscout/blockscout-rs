#[cfg(target_os = "linux")]
pub const DEFAULT_COMPILER_LIST: &str =
    //"https://raw.githubusercontent.com/blockscout/solc-bin/main/list.json";
    "https://solc-bin.ethereum.org/linux-amd64/list.json";
#[cfg(target_os = "macos")]
pub const DEFAULT_COMPILER_LIST: &str = "https://solc-bin.ethereum.org/macosx-amd64/list.json";
#[cfg(target_os = "windows")]
pub const DEFAULT_COMPILER_LIST: &str = "https://solc-bin.ethereum.org/windows-amd64/list.json";
