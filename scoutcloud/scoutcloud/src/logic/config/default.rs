lazy_static::lazy_static! {
    pub static ref DEFAULT_CONFIG: serde_yaml::Value = {
        serde_yaml::from_str(include_str!("default.yaml")).unwrap()
    };
}
