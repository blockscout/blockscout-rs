use std::{
    collections::HashSet,
    fmt::{Display, Formatter},
};
use strum::{EnumIter, IntoEnumIterator};

#[derive(Debug, Clone, Hash, PartialEq, Eq, EnumIter)]
pub enum ResponseFieldMask {
    Svg,
    Png,
}

impl Display for ResponseFieldMask {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ResponseFieldMask::Svg => f.write_str("svg"),
            ResponseFieldMask::Png => f.write_str("png"),
        }
    }
}

impl TryFrom<&str> for ResponseFieldMask {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "svg" => Ok(ResponseFieldMask::Svg),
            "png" => Ok(ResponseFieldMask::Png),
            _ => Err(anyhow::anyhow!("invalid response filed mask: {}", value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutputMask(pub HashSet<ResponseFieldMask>);

impl OutputMask {
    pub fn contains(&self, key: &ResponseFieldMask) -> bool {
        self.0.contains(key)
    }
}

impl OutputMask {
    pub fn full() -> Self {
        OutputMask(ResponseFieldMask::iter().collect())
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Response {
    pub svg: Option<Vec<u8>>,
    pub png: Option<Vec<u8>>,
}
