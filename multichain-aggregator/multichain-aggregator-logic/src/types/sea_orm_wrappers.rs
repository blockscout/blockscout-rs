use alloy_primitives::{Address, B256, Bytes};
use sea_orm::{ColIdx, DbErr, QueryResult, TryGetError, TryGetable};
use std::{ops::Deref, str::FromStr};

macro_rules! impl_sea_orm_wrapper {
    ($type:ty, $db_type:ty, $wrapper:ident, $convert:expr) => {
        #[derive(Debug, Clone)]
        pub struct $wrapper($type);

        impl TryGetable for $wrapper {
            fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
                let v = <$db_type>::try_get_by(res, index)?;
                Ok(Self($convert(v).map_err(TryGetError::DbErr)?))
            }
        }

        impl From<$type> for $wrapper {
            fn from(v: $type) -> Self {
                Self(v)
            }
        }

        impl Deref for $wrapper {
            type Target = $type;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

impl_sea_orm_wrapper!(Address, Vec<u8>, SeaOrmAddress, |v: Vec<u8>| {
    Address::try_from(v.as_slice()).map_err(|_| DbErr::Custom("invalid address".to_string()))
});

impl FromStr for SeaOrmAddress {
    type Err = <Address as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Address::from_str(s)?))
    }
}

impl_sea_orm_wrapper!(B256, Vec<u8>, SeaOrmB256, |v: Vec<u8>| {
    B256::try_from(v.as_slice()).map_err(|_| DbErr::Custom("invalid hash".to_string()))
});

impl FromStr for SeaOrmB256 {
    type Err = <B256 as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(B256::from_str(s)?))
    }
}

impl_sea_orm_wrapper!(Bytes, Vec<u8>, SeaOrmBytes, |v: Vec<u8>| {
    Ok(Bytes::from(v))
});

impl FromStr for SeaOrmBytes {
    type Err = <Bytes as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Bytes::from_str(s)?))
    }
}
