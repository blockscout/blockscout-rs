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

pub trait ParsePageToken<T: PageTokenFormat> {
    #[allow(clippy::result_large_err)]
    fn parse_page_token(self) -> Result<Option<T>, Status>;
}

impl<T: PageTokenFormat> ParsePageToken<T> for Option<String> {
    fn parse_page_token(self) -> Result<Option<T>, Status> {
        self.map(|s| T::from(s.clone()).map_err(|_| Status::invalid_argument("invalid page_token")))
            .transpose()
    }
}

pub fn page_token_to_proto<T: PageTokenFormat>(
    page_token: Option<T>,
    page_size: u32,
) -> Option<Pagination> {
    page_token.map(|pt| Pagination {
        page_token: pt.format(),
        page_size,
    })
}
