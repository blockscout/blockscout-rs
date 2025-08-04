use alloy_primitives::{Address, B256};
use base64::Engine;
use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use sea_orm::prelude::Decimal;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PageTokenParsingError {
    #[error("base64 decode error: {0}")]
    Base64DecodeError(String),
    #[error("json decode error: {0}")]
    JsonDecodeError(String),
    #[error("invalid format, expected {0} values")]
    FormatError(u32),
    #[error("invalid format '{0}' in part #{1}")]
    ParsingError(String, u32),
}

pub trait PageTokenFormat: Sized {
    fn parse_page_token(page_token: String) -> Result<Self, PageTokenParsingError>;
    fn format_page_token(self) -> String;
}

// Encode Option<T> as "{is_some}:{value}" pair
// where is_some=1 if the value is Some(T)
// and is_some=0 if the value is None
impl<T: PageTokenFormat> PageTokenFormat for Option<T> {
    fn parse_page_token(page_token: String) -> Result<Self, PageTokenParsingError> {
        match page_token.split_once(":") {
            Some((is_some, value)) => match is_some {
                "1" => Ok(Some(T::parse_page_token(value.to_string())?)),
                "0" => Ok(None),
                _ => Err(PageTokenParsingError::ParsingError(page_token, 1)),
            },
            None => Err(PageTokenParsingError::ParsingError(page_token, 0)),
        }
    }

    fn format_page_token(self) -> String {
        match self {
            Some(t) => format!("1:{}", t.format_page_token()),
            None => "0:".to_string(),
        }
    }
}

macro_rules! impl_page_token_format {
    ($($type:ty),*) => {
        $(
            impl PageTokenFormat for $type {
                fn parse_page_token(page_token: String) -> Result<Self, PageTokenParsingError> {
                    page_token
                        .parse()
                        .map_err(|_| PageTokenParsingError::ParsingError(page_token, 0))
                }

                fn format_page_token(self) -> String {
                    self.to_string()
                }
            }
        )*
    };
}

impl_page_token_format!(i32, i64, Address, B256, String);

impl PageTokenFormat for Decimal {
    fn parse_page_token(page_token: String) -> Result<Self, PageTokenParsingError> {
        page_token
            .parse()
            .map_err(|_| PageTokenParsingError::ParsingError(page_token, 0))
    }

    fn format_page_token(self) -> String {
        self.normalize().to_string()
    }
}

impl PageTokenFormat for BigDecimal {
    fn parse_page_token(page_token: String) -> Result<Self, PageTokenParsingError> {
        page_token
            .parse()
            .map_err(|_| PageTokenParsingError::ParsingError(page_token, 0))
    }

    fn format_page_token(self) -> String {
        self.to_plain_string()
    }
}

impl PageTokenFormat for NaiveDateTime {
    fn parse_page_token(page_token: String) -> Result<Self, PageTokenParsingError> {
        match page_token
            .parse()
            .map(chrono::DateTime::from_timestamp_micros)
        {
            Ok(Some(dt)) => Ok(dt.naive_utc()),
            _ => Err(PageTokenParsingError::ParsingError(page_token, 0)),
        }
    }

    fn format_page_token(self) -> String {
        self.and_utc().timestamp_micros().to_string()
    }
}

// Tuples are formatted as base64-encoded JSON array of strings
// where each element is formatted as a string using the PageTokenFormat trait
macro_rules! impl_page_token_format_tuple {
    ($len:literal: ($($T:ident $var:ident),+)) => {
        impl<$($T: PageTokenFormat),+> PageTokenFormat for ($($T),+,) {
            fn parse_page_token(page_token: String) -> Result<Self, PageTokenParsingError> {
                let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(page_token)
                    .map_err(|e| PageTokenParsingError::Base64DecodeError(e.to_string()))?;

                let parts: Vec<&str> = serde_json::from_slice(&decoded)
                    .map_err(|e| PageTokenParsingError::JsonDecodeError(e.to_string()))?;

                match parts.as_slice() {
                    &[$($var),+] => Ok((
                        $(
                            $T::parse_page_token($var.to_string())?,
                        )+
                    )),
                    _ => Err(PageTokenParsingError::FormatError($len as u32)),
                }
            }

            fn format_page_token(self) -> String {
                let ($($var),+,) = self;
                let parts = vec![
                    $(
                        $var.format_page_token(),
                    )+
                ];
                let json = serde_json::to_string(&parts).expect("list of strings should be serializable");
                base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json)
            }
        }
    };
}

impl_page_token_format_tuple!(1: (T1 v1));
impl_page_token_format_tuple!(2: (T1 v1, T2 v2));
impl_page_token_format_tuple!(3: (T1 v1, T2 v2, T3 v3));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_token_format() {
        let pt: (i32, String, Option<i64>) = (1, "2,3".to_string(), None);
        let pt_str = pt.clone().format_page_token();
        let pt_parsed = PageTokenFormat::parse_page_token(pt_str).unwrap();
        assert_eq!(pt, pt_parsed);

        let pt: (Decimal, BigDecimal, NaiveDateTime) = (
            "1.2".parse().unwrap(),
            "3.4".parse().unwrap(),
            "2015-09-18T23:56:04".parse().unwrap(),
        );
        let pt_str = pt.clone().format_page_token();
        let pt_parsed = PageTokenFormat::parse_page_token(pt_str).unwrap();
        assert_eq!(pt, pt_parsed);

        let pt: (Address, B256) = (
            "0x1234567890123456789012345678901234567890"
                .parse()
                .unwrap(),
            "0x1234567890123456789012345678901234567890123456789012345678901234"
                .parse()
                .unwrap(),
        );
        let pt_str = pt.format_page_token();
        let pt_parsed = PageTokenFormat::parse_page_token(pt_str).unwrap();
        assert_eq!(pt, pt_parsed);

        let pt: (String,) = ("1".to_string(),);
        let pt_str = pt.clone().format_page_token();
        let pt_parsed = PageTokenFormat::parse_page_token(pt_str).unwrap();
        assert_eq!(pt, pt_parsed);
    }
}
