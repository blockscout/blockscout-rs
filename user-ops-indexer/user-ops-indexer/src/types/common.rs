use atoi::FromRadix10;
use ethers::prelude::U256;
use sea_orm::prelude::Decimal;

pub fn u256_to_decimal(n: U256) -> Decimal {
    Decimal::from_radix_10(n.to_string().as_bytes()).0
}
