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

#[allow(clippy::result_large_err)]
#[inline]
pub fn parse_query_2<T1: FromStr, T2: FromStr>(input: String) -> Result<(T1, T2), Status>
where
    <T1 as FromStr>::Err: std::fmt::Display,
    <T2 as FromStr>::Err: std::fmt::Display,
{
    match input.split(',').collect::<Vec<&str>>().as_slice() {
        [v1, v2] => Ok((
            parse_query::<T1>(v1.to_string())?,
            parse_query::<T2>(v2.to_string())?,
        )),
        _ => Err(Status::invalid_argument("invalid page_token format")),
    }
}
