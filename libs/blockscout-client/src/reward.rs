#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Reward {
    pub reward: i64,
    #[serde(rename = "type")]
    pub type_: String,
}

impl Reward {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> RewardBuilder<crate::generics::MissingReward, crate::generics::MissingType> {
        RewardBuilder {
            body: Default::default(),
            _reward: core::marker::PhantomData,
            _type: core::marker::PhantomData,
        }
    }
}

impl Into<Reward> for RewardBuilder<crate::generics::RewardExists, crate::generics::TypeExists> {
    fn into(self) -> Reward {
        self.body
    }
}

/// Builder for [`Reward`](./struct.Reward.html) object.
#[derive(Debug, Clone)]
pub struct RewardBuilder<Reward, Type> {
    body: self::Reward,
    _reward: core::marker::PhantomData<Reward>,
    _type: core::marker::PhantomData<Type>,
}

impl<Reward, Type> RewardBuilder<Reward, Type> {
    #[inline]
    pub fn reward(mut self, value: impl Into<i64>) -> RewardBuilder<crate::generics::RewardExists, Type> {
        self.body.reward = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn type_(mut self, value: impl Into<String>) -> RewardBuilder<Reward, crate::generics::TypeExists> {
        self.body.type_ = value.into();
        unsafe { std::mem::transmute(self) }
    }
}
