#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TransactionReward {
    pub block_hash: String,
    pub emission_reward: String,
    pub from: crate::address_param::AddressParam,
    pub to: crate::address_param::AddressParam,
    pub types: crate::transaction_reward::TransactionRewardTypes,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TransactionRewardTypes {}

impl TransactionReward {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> TransactionRewardBuilder<crate::generics::MissingBlockHash, crate::generics::MissingEmissionReward, crate::generics::MissingFrom, crate::generics::MissingTo, crate::generics::MissingTypes> {
        TransactionRewardBuilder {
            body: Default::default(),
            _block_hash: core::marker::PhantomData,
            _emission_reward: core::marker::PhantomData,
            _from: core::marker::PhantomData,
            _to: core::marker::PhantomData,
            _types: core::marker::PhantomData,
        }
    }
}

impl Into<TransactionReward> for TransactionRewardBuilder<crate::generics::BlockHashExists, crate::generics::EmissionRewardExists, crate::generics::FromExists, crate::generics::ToExists, crate::generics::TypesExists> {
    fn into(self) -> TransactionReward {
        self.body
    }
}

/// Builder for [`TransactionReward`](./struct.TransactionReward.html) object.
#[derive(Debug, Clone)]
pub struct TransactionRewardBuilder<BlockHash, EmissionReward, From, To, Types> {
    body: self::TransactionReward,
    _block_hash: core::marker::PhantomData<BlockHash>,
    _emission_reward: core::marker::PhantomData<EmissionReward>,
    _from: core::marker::PhantomData<From>,
    _to: core::marker::PhantomData<To>,
    _types: core::marker::PhantomData<Types>,
}

impl<BlockHash, EmissionReward, From, To, Types> TransactionRewardBuilder<BlockHash, EmissionReward, From, To, Types> {
    #[inline]
    pub fn block_hash(mut self, value: impl Into<String>) -> TransactionRewardBuilder<crate::generics::BlockHashExists, EmissionReward, From, To, Types> {
        self.body.block_hash = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn emission_reward(mut self, value: impl Into<String>) -> TransactionRewardBuilder<BlockHash, crate::generics::EmissionRewardExists, From, To, Types> {
        self.body.emission_reward = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn from(mut self, value: crate::address_param::AddressParamBuilder<crate::generics::HashExists>) -> TransactionRewardBuilder<BlockHash, EmissionReward, crate::generics::FromExists, To, Types> {
        self.body.from = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn to(mut self, value: crate::address_param::AddressParamBuilder<crate::generics::HashExists>) -> TransactionRewardBuilder<BlockHash, EmissionReward, From, crate::generics::ToExists, Types> {
        self.body.to = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn types(mut self, value: crate::transaction_reward::TransactionRewardTypes) -> TransactionRewardBuilder<BlockHash, EmissionReward, From, To, crate::generics::TypesExists> {
        self.body.types = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

