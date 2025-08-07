use multichain_aggregator_logic::page_token::PageTokenFormat;
use multichain_aggregator_proto::blockscout::multichain_aggregator::v1::Pagination;
use std::str::FromStr;
use tonic::Status;

#[allow(clippy::result_large_err)]
#[inline]
pub fn parse_query<T: FromStr>(input: String) -> Result<T, Status>
where
    <T as FromStr>::Err: std::fmt::Display,
{
    T::from_str(&input).map_err(|e| Status::invalid_argument(format!("invalid value {input}: {e}")))
}

pub trait PageTokenExtractor<T: PageTokenFormat> {
    #[allow(clippy::result_large_err)]
    fn extract_page_token(self) -> Result<Option<T>, Status>;
}

impl<T: PageTokenFormat> PageTokenExtractor<T> for Option<String> {
    fn extract_page_token(self) -> Result<Option<T>, Status> {
        self.map(|s| {
            T::parse_page_token(s.clone())
                .map_err(|e| Status::invalid_argument(format!("invalid page_token: {e}")))
        })
        .transpose()
    }
}

pub fn page_token_to_proto<T: PageTokenFormat>(
    page_token: Option<T>,
    page_size: u32,
) -> Option<Pagination> {
    page_token.map(|pt| Pagination {
        page_token: pt.format_page_token(),
        page_size,
    })
}
