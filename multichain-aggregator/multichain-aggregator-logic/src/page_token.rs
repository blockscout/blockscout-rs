use alloy_primitives::{Address, B256};
use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use sea_orm::prelude::Decimal;
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

// Encode Option<T> as "{is_some}:{value}" pair
// where is_some=1 if the value is Some(T)
// and is_some=0 if the value is None
impl<T: PageTokenFormat> PageTokenFormat for Option<T> {
    fn from(page_token: String) -> Result<Self, PageTokenParsingError> {
        match page_token.split_once(":") {
            Some((is_some, value)) => match is_some {
                "1" => Ok(Some(T::from(value.to_string())?)),
                "0" => Ok(None),
                _ => Err(PageTokenParsingError::ParsingError(page_token, 1)),
            },
            None => Err(PageTokenParsingError::ParsingError(page_token, 0)),
        }
    }

    fn format(self) -> String {
        match self {
            Some(t) => format!("1:{}", t.format()),
            None => "0:".to_string(),
        }
    }
}

impl PageTokenFormat for String {
    fn from(page_token: String) -> Result<Self, PageTokenParsingError> {
        Ok(page_token)
    }

    fn format(self) -> String {
        self
    }
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

impl_page_token_format!(i32, i64, Address, B256);

impl PageTokenFormat for Decimal {
    fn from(page_token: String) -> Result<Self, PageTokenParsingError> {
        page_token
            .parse()
            .map_err(|_| PageTokenParsingError::ParsingError(page_token, 0))
    }

    fn format(self) -> String {
        self.normalize().to_string()
    }
}

impl PageTokenFormat for BigDecimal {
    fn from(page_token: String) -> Result<Self, PageTokenParsingError> {
        page_token
            .parse()
            .map_err(|_| PageTokenParsingError::ParsingError(page_token, 0))
    }

    fn format(self) -> String {
        self.to_plain_string()
    }
}

impl PageTokenFormat for NaiveDateTime {
    fn from(page_token: String) -> Result<Self, PageTokenParsingError> {
        match page_token
            .parse()
            .map(chrono::DateTime::from_timestamp_micros)
        {
            Ok(Some(dt)) => Ok(dt.naive_utc()),
            _ => Err(PageTokenParsingError::ParsingError(page_token, 0)),
        }
    }

    fn format(self) -> String {
        self.and_utc().timestamp_micros().to_string()
    }
}

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
