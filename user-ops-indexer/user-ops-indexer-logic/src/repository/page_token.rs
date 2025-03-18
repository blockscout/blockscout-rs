use alloy::primitives::{Address, B256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PageTokenParsingError {
    #[error("invalid format, expected {0} comma-separated values")]
    FormatError(u32),
    #[error("invalid format '{0}' in part #{1}")]
    ParsingError(String, u32),
}

pub trait PageTokenFormat: Sized {
    fn from(page_token: String) -> Result<Self, PageTokenParsingError>;
    fn format(self) -> String;
}

macro_rules! impl_page_token_format {
    ($($type:ty),*) => {
        $(
            impl PageTokenFormat for $type {
                fn from(page_token: String) -> Result<Self, PageTokenParsingError> {
                    page_token
                        .parse()
                        .map_err(|_| PageTokenParsingError::ParsingError(page_token, 0))
                }

                fn format(self) -> String {
                    self.to_string()
                }
            }
        )*
    };
}

impl_page_token_format!(u32, u64, Address, B256);

impl<T1: PageTokenFormat, T2: PageTokenFormat> PageTokenFormat for (T1, T2) {
    fn from(page_token: String) -> Result<Self, PageTokenParsingError> {
        match page_token.split(',').collect::<Vec<&str>>().as_slice() {
            &[v1, v2] => Ok((T1::from(v1.to_string())?, T2::from(v2.to_string())?)),
            _ => Err(PageTokenParsingError::FormatError(2)),
        }
    }

    fn format(self) -> String {
        format!("{},{}", self.0.format(), self.1.format())
    }
}

impl<T1: PageTokenFormat, T2: PageTokenFormat, T3: PageTokenFormat> PageTokenFormat
    for (T1, T2, T3)
{
    fn from(page_token: String) -> Result<Self, PageTokenParsingError> {
        match page_token.split(',').collect::<Vec<&str>>().as_slice() {
            &[v1, v2, v3] => Ok((
                T1::from(v1.to_string())?,
                T2::from(v2.to_string())?,
                T3::from(v3.to_string())?,
            )),
            _ => Err(PageTokenParsingError::FormatError(3)),
        }
    }

    fn format(self) -> String {
        format!(
            "{},{},{}",
            self.0.format(),
            self.1.format(),
            self.2.format()
        )
    }
}
