use alloy_primitives::{Address, B256, Bytes};
use amplify::Wrapper;
use sea_orm::{ColIdx, DbErr, QueryResult, TryGetError, TryGetable};
use serde::{Deserialize, Serialize};

macro_rules! impl_sea_orm_wrapper {
    ($type:ty, $db_type:ty, $wrapper:ident, $convert:expr) => {
        #[derive(Wrapper, Debug, Clone, Serialize, Deserialize)]
        #[wrapper(Deref, FromStr)]
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
    };
}

impl_sea_orm_wrapper!(Address, Vec<u8>, SeaOrmAddress, |v: Vec<u8>| {
    Address::try_from(v.as_slice()).map_err(|_| DbErr::Custom("invalid address".to_string()))
});

impl_sea_orm_wrapper!(B256, Vec<u8>, SeaOrmB256, |v: Vec<u8>| {
    B256::try_from(v.as_slice()).map_err(|_| DbErr::Custom("invalid hash".to_string()))
});

impl_sea_orm_wrapper!(Bytes, Vec<u8>, SeaOrmBytes, |v: Vec<u8>| {
    Ok(Bytes::from(v))
});
