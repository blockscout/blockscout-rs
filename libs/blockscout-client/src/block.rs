#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Block {
    pub base_fee_per_gas: Option<i64>,
    pub burnt_fees: Option<i64>,
    pub burnt_fees_percentage: Option<f64>,
    pub difficulty: i64,
    pub extra_data: Option<String>,
    pub gas_limit: i64,
    pub gas_target_percentage: Option<f64>,
    pub gas_used: i64,
    pub gas_used_percentage: Option<f64>,
    pub hash: String,
    pub height: i64,
    pub miner: crate::address_param::AddressParam,
    pub nonce: String,
    pub parent_hash: String,
    pub priority_fee: Option<i64>,
    pub rewards: Option<Vec<crate::reward::Reward>>,
    pub size: i64,
    pub state_root: Option<String>,
    pub timestamp: String,
    pub total_difficulty: i64,
    pub tx_count: i64,
    pub tx_fees: Option<i64>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub uncles_hashes: Option<crate::block::BlockUnclesHashes>,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BlockUnclesHashes {}

impl Block {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> BlockBuilder<crate::generics::MissingDifficulty, crate::generics::MissingGasLimit, crate::generics::MissingGasUsed, crate::generics::MissingHash, crate::generics::MissingHeight, crate::generics::MissingMiner, crate::generics::MissingNonce, crate::generics::MissingParentHash, crate::generics::MissingSize, crate::generics::MissingTimestamp, crate::generics::MissingTotalDifficulty, crate::generics::MissingTxCount> {
        BlockBuilder {
            body: Default::default(),
            _difficulty: core::marker::PhantomData,
            _gas_limit: core::marker::PhantomData,
            _gas_used: core::marker::PhantomData,
            _hash: core::marker::PhantomData,
            _height: core::marker::PhantomData,
            _miner: core::marker::PhantomData,
            _nonce: core::marker::PhantomData,
            _parent_hash: core::marker::PhantomData,
            _size: core::marker::PhantomData,
            _timestamp: core::marker::PhantomData,
            _total_difficulty: core::marker::PhantomData,
            _tx_count: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_block() -> BlockGetBuilder<crate::generics::MissingBlockNumberOrHash> {
        BlockGetBuilder {
            inner: Default::default(),
            _param_block_number_or_hash: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_main_page_tokens() -> BlockGetBuilder1 {
        BlockGetBuilder1
    }
}

impl Into<Block> for BlockBuilder<crate::generics::DifficultyExists, crate::generics::GasLimitExists, crate::generics::GasUsedExists, crate::generics::HashExists, crate::generics::HeightExists, crate::generics::MinerExists, crate::generics::NonceExists, crate::generics::ParentHashExists, crate::generics::SizeExists, crate::generics::TimestampExists, crate::generics::TotalDifficultyExists, crate::generics::TxCountExists> {
    fn into(self) -> Block {
        self.body
    }
}

/// Builder for [`Block`](./struct.Block.html) object.
#[derive(Debug, Clone)]
pub struct BlockBuilder<Difficulty, GasLimit, GasUsed, Hash, Height, Miner, Nonce, ParentHash, Size, Timestamp, TotalDifficulty, TxCount> {
    body: self::Block,
    _difficulty: core::marker::PhantomData<Difficulty>,
    _gas_limit: core::marker::PhantomData<GasLimit>,
    _gas_used: core::marker::PhantomData<GasUsed>,
    _hash: core::marker::PhantomData<Hash>,
    _height: core::marker::PhantomData<Height>,
    _miner: core::marker::PhantomData<Miner>,
    _nonce: core::marker::PhantomData<Nonce>,
    _parent_hash: core::marker::PhantomData<ParentHash>,
    _size: core::marker::PhantomData<Size>,
    _timestamp: core::marker::PhantomData<Timestamp>,
    _total_difficulty: core::marker::PhantomData<TotalDifficulty>,
    _tx_count: core::marker::PhantomData<TxCount>,
}

impl<Difficulty, GasLimit, GasUsed, Hash, Height, Miner, Nonce, ParentHash, Size, Timestamp, TotalDifficulty, TxCount> BlockBuilder<Difficulty, GasLimit, GasUsed, Hash, Height, Miner, Nonce, ParentHash, Size, Timestamp, TotalDifficulty, TxCount> {
    #[inline]
    pub fn base_fee_per_gas(mut self, value: impl Into<i64>) -> Self {
        self.body.base_fee_per_gas = Some(value.into());
        self
    }

    #[inline]
    pub fn burnt_fees(mut self, value: impl Into<i64>) -> Self {
        self.body.burnt_fees = Some(value.into());
        self
    }

    #[inline]
    pub fn burnt_fees_percentage(mut self, value: impl Into<f64>) -> Self {
        self.body.burnt_fees_percentage = Some(value.into());
        self
    }

    #[inline]
    pub fn difficulty(mut self, value: impl Into<i64>) -> BlockBuilder<crate::generics::DifficultyExists, GasLimit, GasUsed, Hash, Height, Miner, Nonce, ParentHash, Size, Timestamp, TotalDifficulty, TxCount> {
        self.body.difficulty = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn extra_data(mut self, value: impl Into<String>) -> Self {
        self.body.extra_data = Some(value.into());
        self
    }

    #[inline]
    pub fn gas_limit(mut self, value: impl Into<i64>) -> BlockBuilder<Difficulty, crate::generics::GasLimitExists, GasUsed, Hash, Height, Miner, Nonce, ParentHash, Size, Timestamp, TotalDifficulty, TxCount> {
        self.body.gas_limit = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn gas_target_percentage(mut self, value: impl Into<f64>) -> Self {
        self.body.gas_target_percentage = Some(value.into());
        self
    }

    #[inline]
    pub fn gas_used(mut self, value: impl Into<i64>) -> BlockBuilder<Difficulty, GasLimit, crate::generics::GasUsedExists, Hash, Height, Miner, Nonce, ParentHash, Size, Timestamp, TotalDifficulty, TxCount> {
        self.body.gas_used = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn gas_used_percentage(mut self, value: impl Into<f64>) -> Self {
        self.body.gas_used_percentage = Some(value.into());
        self
    }

    #[inline]
    pub fn hash(mut self, value: impl Into<String>) -> BlockBuilder<Difficulty, GasLimit, GasUsed, crate::generics::HashExists, Height, Miner, Nonce, ParentHash, Size, Timestamp, TotalDifficulty, TxCount> {
        self.body.hash = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn height(mut self, value: impl Into<i64>) -> BlockBuilder<Difficulty, GasLimit, GasUsed, Hash, crate::generics::HeightExists, Miner, Nonce, ParentHash, Size, Timestamp, TotalDifficulty, TxCount> {
        self.body.height = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn miner(mut self, value: crate::address_param::AddressParamBuilder<crate::generics::HashExists>) -> BlockBuilder<Difficulty, GasLimit, GasUsed, Hash, Height, crate::generics::MinerExists, Nonce, ParentHash, Size, Timestamp, TotalDifficulty, TxCount> {
        self.body.miner = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn nonce(mut self, value: impl Into<String>) -> BlockBuilder<Difficulty, GasLimit, GasUsed, Hash, Height, Miner, crate::generics::NonceExists, ParentHash, Size, Timestamp, TotalDifficulty, TxCount> {
        self.body.nonce = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn parent_hash(mut self, value: impl Into<String>) -> BlockBuilder<Difficulty, GasLimit, GasUsed, Hash, Height, Miner, Nonce, crate::generics::ParentHashExists, Size, Timestamp, TotalDifficulty, TxCount> {
        self.body.parent_hash = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn priority_fee(mut self, value: impl Into<i64>) -> Self {
        self.body.priority_fee = Some(value.into());
        self
    }

    #[inline]
    pub fn rewards(mut self, value: impl Iterator<Item = crate::reward::RewardBuilder<crate::generics::RewardExists, crate::generics::TypeExists>>) -> Self {
        self.body.rewards = Some(value.map(|value| value.into()).collect::<Vec<_>>().into());
        self
    }

    #[inline]
    pub fn size(mut self, value: impl Into<i64>) -> BlockBuilder<Difficulty, GasLimit, GasUsed, Hash, Height, Miner, Nonce, ParentHash, crate::generics::SizeExists, Timestamp, TotalDifficulty, TxCount> {
        self.body.size = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn state_root(mut self, value: impl Into<String>) -> Self {
        self.body.state_root = Some(value.into());
        self
    }

    #[inline]
    pub fn timestamp(mut self, value: impl Into<String>) -> BlockBuilder<Difficulty, GasLimit, GasUsed, Hash, Height, Miner, Nonce, ParentHash, Size, crate::generics::TimestampExists, TotalDifficulty, TxCount> {
        self.body.timestamp = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn total_difficulty(mut self, value: impl Into<i64>) -> BlockBuilder<Difficulty, GasLimit, GasUsed, Hash, Height, Miner, Nonce, ParentHash, Size, Timestamp, crate::generics::TotalDifficultyExists, TxCount> {
        self.body.total_difficulty = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn tx_count(mut self, value: impl Into<i64>) -> BlockBuilder<Difficulty, GasLimit, GasUsed, Hash, Height, Miner, Nonce, ParentHash, Size, Timestamp, TotalDifficulty, crate::generics::TxCountExists> {
        self.body.tx_count = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn tx_fees(mut self, value: impl Into<i64>) -> Self {
        self.body.tx_fees = Some(value.into());
        self
    }

    #[inline]
    pub fn type_(mut self, value: impl Into<String>) -> Self {
        self.body.type_ = Some(value.into());
        self
    }

    #[inline]
    pub fn uncles_hashes(mut self, value: crate::block::BlockUnclesHashes) -> Self {
        self.body.uncles_hashes = Some(value.into());
        self
    }
}

/// Builder created by [`Block::get_block`](./struct.Block.html#method.get_block) method for a `GET` operation associated with `Block`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct BlockGetBuilder<BlockNumberOrHash> {
    inner: BlockGetBuilderContainer,
    _param_block_number_or_hash: core::marker::PhantomData<BlockNumberOrHash>,
}

#[derive(Debug, Default, Clone)]
struct BlockGetBuilderContainer {
    param_block_number_or_hash: Option<String>,
}

impl<BlockNumberOrHash> BlockGetBuilder<BlockNumberOrHash> {
    /// Block number or hash
    #[inline]
    pub fn block_number_or_hash(mut self, value: impl Into<String>) -> BlockGetBuilder<crate::generics::BlockNumberOrHashExists> {
        self.inner.param_block_number_or_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for BlockGetBuilder<crate::generics::BlockNumberOrHashExists> {
    type Output = Block;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/blocks/{block_number_or_hash}", block_number_or_hash=self.inner.param_block_number_or_hash.as_ref().expect("missing parameter block_number_or_hash?")).into()
    }
}

/// Builder created by [`Block::get_main_page_tokens`](./struct.Block.html#method.get_main_page_tokens) method for a `GET` operation associated with `Block`.
#[derive(Debug, Clone)]
pub struct BlockGetBuilder1;


impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for BlockGetBuilder1 {
    type Output = Vec<Block>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        "/main-page/blocks".into()
    }
}

