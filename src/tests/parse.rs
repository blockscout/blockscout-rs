use std::fmt::Debug;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

pub fn test_deserialize_ok<T>(tests: Vec<(&str, T)>)
where
    T: Debug + PartialEq + DeserializeOwned,
{
    for (s, value) in tests {
        let v: T = serde_json::from_str(s).unwrap();
        assert_eq!(v, value);
    }
}

pub fn test_serialize_json_ok<T>(tests: Vec<(T, Value)>)
where
    T: Serialize,
{
    for (object, expected_json) in tests {
        let object_string = serde_json::to_string(&object).unwrap();
        let object_json: Value = serde_json::from_str(&object_string).unwrap();

        assert_eq!(object_json, expected_json);
    }
}
