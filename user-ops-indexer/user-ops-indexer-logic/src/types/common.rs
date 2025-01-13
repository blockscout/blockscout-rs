use alloy::primitives::U256;
use sea_orm::prelude::BigDecimal;
use std::str::FromStr;

pub fn u256_to_decimal(n: U256) -> BigDecimal {
    BigDecimal::from_str(&n.to_string()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u256_to_decimal() {
        assert_eq!(u256_to_decimal(U256::from(0)), BigDecimal::from(0));
        assert_eq!(u256_to_decimal(U256::from(1)), BigDecimal::from(1));
        assert_eq!(u256_to_decimal(U256::from(1000)), BigDecimal::from(1000));
        assert_eq!(
            u256_to_decimal(U256::MAX).to_string(),
            "115792089237316195423570985008687907853269984665640564039457584007913129639935"
        );
    }
}
