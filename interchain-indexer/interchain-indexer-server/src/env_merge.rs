// SPDX-License-Identifier: LicenseRef-Blockscout

//! Deep-merge of environment-variable overrides into config JSON.
//!
//! Applied between reading `chains.json` / `bridges.json` and their typed
//! deserialization: vars under a dedicated prefix are parsed as JSON fragments
//! (fallback: plain string) and merged into the file's `serde_json::Value`.
//! Arrays are addressed element-wise through [`ArrayRules`] aligned with the
//! DB uniqueness keys, so entries that merge together are exactly the entries
//! that upsert to the same DB row.
//!
//! Env var grammar (`<PREFIX>` is e.g. `INTERCHAIN_INDEXER_CHAINS`):
//!
//! ```text
//! <PREFIX>                                  = whole-config array patch (JSON array)
//! <PREFIX>__<ID>                            = one entry (JSON object fragment)
//! <PREFIX>__<ID>__<FIELD>[__<FIELD>…]       = one field (scalar/fragment)
//! ```

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Map, Value};
use std::collections::HashMap;

pub(crate) enum ArrayRule {
    /// Array of objects addressed by 1..N id fields; the env path supplies one
    /// segment per field, in order. No match → append with the ids injected.
    Keyed(&'static [&'static str]),
    /// `Vec<Map<String, _>>` addressed by map key (the `rpcs` shape).
    /// Unmatched key → inserted into the first map (a `{}` map is pushed
    /// when the vec is empty).
    NamedMap,
}

/// (canonical field path from the root, rule); arrays are transparent in the
/// path, i.e. keyed/named segments do not contribute to it.
pub(crate) struct ArrayRules(pub &'static [(&'static [&'static str], ArrayRule)]);

impl ArrayRules {
    fn get(&self, path: &[String]) -> Option<&ArrayRule> {
        self.0
            .iter()
            .find(|(rule_path, _)| {
                rule_path.len() == path.len() && rule_path.iter().zip(path).all(|(a, b)| *a == b)
            })
            .map(|(_, rule)| rule)
    }
}

pub(crate) const CHAINS_RULES: ArrayRules = ArrayRules(&[
    (&[], ArrayRule::Keyed(&["chain_id"])),
    (&["rpcs"], ArrayRule::NamedMap),
]);

pub(crate) const BRIDGES_RULES: ArrayRules = ArrayRules(&[
    (&[], ArrayRule::Keyed(&["bridge_id"])),
    (
        &["contracts"],
        ArrayRule::Keyed(&["chain_id", "address", "version"]),
    ),
]);

#[derive(Debug)]
pub(crate) struct AppliedOverride {
    pub var: String,
    pub json_path: String,
}

struct Patch {
    var: String,
    segments: Vec<String>,
    value: Value,
}

/// Collect all `<prefix>`/`<prefix>__…` vars, parse their values as JSON
/// (fallback: plain string) and deep-merge them into `root` in
/// `(path depth, var name)` order, so shallow fragments land first and deeper
/// field-level vars refine them.
pub(crate) fn apply_env_overrides(
    root: &mut Value,
    prefix: &str,
    vars: impl Iterator<Item = (String, String)>,
    rules: &ArrayRules,
) -> Result<Vec<AppliedOverride>> {
    let nested_prefix = format!("{prefix}__");
    let mut patches = Vec::new();
    for (name, raw) in vars {
        let segments: Vec<String> = if name == prefix {
            Vec::new()
        } else if let Some(rest) = name.strip_prefix(&nested_prefix) {
            rest.split("__").map(str::to_lowercase).collect()
        } else {
            continue;
        };
        ensure!(
            segments.iter().all(|segment| !segment.is_empty()),
            "malformed env var name '{name}': empty path segment"
        );
        let value = serde_json::from_str::<Value>(&raw).unwrap_or(Value::String(raw));
        patches.push(Patch {
            var: name,
            segments,
            value,
        });
    }

    {
        let mut seen: HashMap<&[String], &str> = HashMap::new();
        for patch in &patches {
            if let Some(previous) = seen.insert(patch.segments.as_slice(), patch.var.as_str()) {
                bail!(
                    "ambiguous env overrides: '{previous}' and '{}' resolve to the same config path '{}'",
                    patch.var,
                    patch.segments.join(".")
                );
            }
        }
    }

    patches.sort_by(|a, b| (a.segments.len(), &a.var).cmp(&(b.segments.len(), &b.var)));

    patches
        .into_iter()
        .map(|patch| {
            let json_path =
                apply_patch(root, &patch.segments, patch.value, &[], rules, &patch.var)?;
            Ok(AppliedOverride {
                var: patch.var,
                json_path,
            })
        })
        .collect()
}

/// Walk `segments` down from `node`, creating missing containers on demand,
/// and merge `value` at the addressed location. Returns the JSON path the
/// patch was applied at (for logging).
fn apply_patch(
    node: &mut Value,
    segments: &[String],
    value: Value,
    rule_path: &[String],
    rules: &ArrayRules,
    var: &str,
) -> Result<String> {
    if segments.is_empty() {
        // Bare-prefix whole-config patch: keyed upsert into the root array.
        let Some(ArrayRule::Keyed(fields)) = rules.get(&[]) else {
            bail!("env var {var}: no root array addressing rule");
        };
        let Value::Array(items) = node else {
            bail!("env var {var}: root config must be a JSON array");
        };
        upsert_root_array(items, value, fields, var)?;
        return Ok("$".to_string());
    }

    match node {
        Value::Array(items) => {
            let rule = rules.get(rule_path).with_context(|| {
                format!(
                    "env var {var}: no array addressing rule for '{}'",
                    rule_path.join(".")
                )
            })?;
            match rule {
                ArrayRule::Keyed(fields) => {
                    apply_to_keyed_array(items, segments, value, fields, rule_path, rules, var)
                }
                ArrayRule::NamedMap => {
                    apply_to_named_map_array(items, segments, value, rule_path, rules, var)
                }
            }
        }
        Value::Object(map) => {
            let field = segments[0].clone();
            if segments.len() == 1 {
                merge_into(map.entry(field.clone()).or_insert(Value::Null), value);
                return Ok(field);
            }
            let child_rule_path: Vec<String> = rule_path
                .iter()
                .cloned()
                .chain(std::iter::once(field.clone()))
                .collect();
            let child = map.entry(field.clone()).or_insert(Value::Null);
            if !child.is_object() && !child.is_array() {
                *child = match rules.get(&child_rule_path) {
                    Some(_) => Value::Array(Vec::new()),
                    None => Value::Object(Map::new()),
                };
            }
            let sub = apply_patch(child, &segments[1..], value, &child_rule_path, rules, var)?;
            Ok(join_path(&field, &sub))
        }
        _ => bail!(
            "env var {var}: cannot descend into a non-container JSON value at '{}'",
            rule_path.join(".")
        ),
    }
}

/// Address one element of a keyed array by consuming `fields.len()` key
/// segments; descend into (or append) the matching element.
fn apply_to_keyed_array(
    items: &mut Vec<Value>,
    segments: &[String],
    value: Value,
    fields: &[&str],
    rule_path: &[String],
    rules: &ArrayRules,
    var: &str,
) -> Result<String> {
    ensure!(
        segments.len() >= fields.len(),
        "env var {var}: array at '{}' is addressed by {} key segment(s) ({}), found only {}",
        rule_path.join("."),
        fields.len(),
        fields.join(", "),
        segments.len()
    );
    let keys: Vec<Value> = segments[..fields.len()]
        .iter()
        .map(|segment| coerce_key_value(segment))
        .collect();
    let matches: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, element)| {
            fields
                .iter()
                .zip(&keys)
                .all(|(field, key)| element.get(*field).is_some_and(|v| json_key_eq(v, key)))
        })
        .map(|(index, _)| index)
        .collect();
    let path_seg = format!(
        "[{}]",
        fields
            .iter()
            .zip(&keys)
            .map(|(field, key)| format!("{field}={}", key_display(key)))
            .collect::<Vec<_>>()
            .join(",")
    );
    let idx = match matches.as_slice() {
        [] => {
            let element: Map<String, Value> = fields
                .iter()
                .zip(&keys)
                .map(|(field, key)| (field.to_string(), key.clone()))
                .collect();
            items.push(Value::Object(element));
            items.len() - 1
        }
        [index] => *index,
        _ => bail!("env var {var}: multiple array elements match key {path_seg}"),
    };
    let rest = &segments[fields.len()..];
    if rest.is_empty() {
        ensure!(
            !value.is_null(),
            "env var {var}: null is not a valid entry override (deletion is not supported)"
        );
        ensure!(
            value.is_object(),
            "env var {var}: entry override for {path_seg} must be a JSON object"
        );
        ensure_fragment_keys_consistent(&value, fields, &keys, var, &path_seg)?;
        merge_into(&mut items[idx], value);
        return Ok(path_seg);
    }
    // A patch may target a key field directly only with the value the element
    // is addressed by; anything else would silently retarget the entry.
    if let Some(position) = fields.iter().position(|field| *field == rest[0]) {
        ensure!(
            rest.len() == 1,
            "env var {var}: cannot descend into key field '{}' of {path_seg}",
            rest[0]
        );
        ensure!(
            json_key_eq(&value, &keys[position]),
            "env var {var}: key field '{}' value conflicts with the addressed entry {path_seg}",
            rest[0]
        );
    }
    let sub = apply_patch(&mut items[idx], rest, value, rule_path, rules, var)?;
    Ok(join_path(&path_seg, &sub))
}

/// Reject an entry fragment whose id fields contradict the key the entry is
/// addressed by (omitting them is fine — they are injected from the path).
fn ensure_fragment_keys_consistent(
    fragment: &Value,
    fields: &[&str],
    keys: &[Value],
    var: &str,
    path_seg: &str,
) -> Result<()> {
    let Value::Object(map) = fragment else {
        return Ok(());
    };
    for (field, key) in fields.iter().zip(keys) {
        if let Some(present) = map.get(*field) {
            ensure!(
                json_key_eq(present, key),
                "env var {var}: fragment field '{field}' ({}) conflicts with the addressed entry {path_seg}",
                key_display(present)
            );
        }
    }
    Ok(())
}

/// Address one entry of a `Vec<Map<name, _>>` array by map key, searched
/// case-insensitively across the flattened maps.
fn apply_to_named_map_array(
    items: &mut Vec<Value>,
    segments: &[String],
    value: Value,
    rule_path: &[String],
    rules: &ArrayRules,
    var: &str,
) -> Result<String> {
    let key = &segments[0];
    let mut found: Vec<(usize, String)> = Vec::new();
    for (index, element) in items.iter().enumerate() {
        let Value::Object(map) = element else {
            bail!(
                "env var {var}: named-map array at '{}' contains a non-object element",
                rule_path.join(".")
            );
        };
        found.extend(
            map.keys()
                .filter(|existing| existing.eq_ignore_ascii_case(key))
                .map(|existing| (index, existing.clone())),
        );
    }
    let (map_idx, actual_key) = match found.as_slice() {
        [] => {
            if items.is_empty() {
                items.push(Value::Object(Map::new()));
            }
            match &mut items[0] {
                Value::Object(first) => {
                    first.insert(key.clone(), Value::Null);
                }
                // All elements were validated as objects above.
                _ => bail!(
                    "env var {var}: named-map array at '{}' contains a non-object element",
                    rule_path.join(".")
                ),
            }
            (0, key.clone())
        }
        [(index, existing)] => (*index, existing.clone()),
        _ => bail!(
            "env var {var}: name '{key}' matches multiple entries at '{}'",
            rule_path.join(".")
        ),
    };
    let entry = items[map_idx]
        .get_mut(&actual_key)
        .expect("key was just found or inserted");
    let path_seg = format!("[{actual_key}]");
    let rest = &segments[1..];
    if rest.is_empty() {
        ensure!(
            !value.is_null(),
            "env var {var}: null is not a valid entry override (deletion is not supported)"
        );
        merge_into(entry, value);
        return Ok(path_seg);
    }
    if !entry.is_object() && !entry.is_array() {
        *entry = Value::Object(Map::new());
    }
    let sub = apply_patch(entry, rest, value, rule_path, rules, var)?;
    Ok(join_path(&path_seg, &sub))
}

/// Bare-prefix root patch: each element of the JSON array is upserted
/// (deep-merged into the element with the same id fields, or appended).
fn upsert_root_array(
    items: &mut Vec<Value>,
    value: Value,
    fields: &[&str],
    var: &str,
) -> Result<()> {
    let Value::Array(patch_items) = value else {
        bail!("env var {var}: whole-config patch must be a JSON array");
    };
    for element in patch_items {
        ensure!(
            element.is_object(),
            "env var {var}: whole-config patch elements must be JSON objects"
        );
        let keys: Vec<Value> = fields
            .iter()
            .map(|field| {
                element.get(*field).cloned().with_context(|| {
                    format!("env var {var}: patch element is missing the id field '{field}'")
                })
            })
            .collect::<Result<_>>()?;
        let matches: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, existing)| {
                fields
                    .iter()
                    .zip(&keys)
                    .all(|(field, key)| existing.get(*field).is_some_and(|v| json_key_eq(v, key)))
            })
            .map(|(index, _)| index)
            .collect();
        match matches.as_slice() {
            [] => items.push(element),
            [index] => merge_into(&mut items[*index], element),
            _ => bail!(
                "env var {var}: multiple existing elements match id fields ({})",
                keys.iter().map(key_display).collect::<Vec<_>>().join(",")
            ),
        }
    }
    Ok(())
}

/// Objects deep-merge recursively; any other combination replaces the target.
/// `null` replaces the value but keeps the key: several config fields are
/// `Option` without `#[serde(default)]`, so key removal would surface as
/// `missing field` errors from the typed parse.
fn merge_into(target: &mut Value, value: Value) {
    match value {
        Value::Object(fragment) if target.is_object() => {
            let map = target.as_object_mut().expect("checked is_object above");
            for (key, val) in fragment {
                match map.get_mut(&key) {
                    Some(existing) => merge_into(existing, val),
                    None => {
                        map.insert(key, val);
                    }
                }
            }
        }
        value => *target = value,
    }
}

/// Integer segments become JSON numbers so keyed matching against file
/// numbers works; everything else stays a (lowercased) string.
fn coerce_key_value(segment: &str) -> Value {
    segment
        .parse::<i64>()
        .map(Value::from)
        .unwrap_or_else(|_| Value::String(segment.to_string()))
}

/// Key equality: numbers numerically, strings case-insensitively.
fn json_key_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_i64(), y.as_i64()) {
            (Some(x), Some(y)) => x == y,
            _ => x == y,
        },
        (Value::String(x), Value::String(y)) => x.eq_ignore_ascii_case(y),
        _ => false,
    }
}

fn key_display(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn join_path(parent: &str, child: &str) -> String {
    match child.starts_with('[') {
        true => format!("{parent}{child}"),
        false => format!("{parent}.{child}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CHAINS: &str = "INTERCHAIN_INDEXER_CHAINS";
    const BRIDGES: &str = "INTERCHAIN_INDEXER_BRIDGES";

    fn chains_fixture() -> Value {
        json!([{
            "chain_id": 1,
            "name": "Ethereum",
            "icon": "https://icon.example/eth.svg",
            "rpcs": [{
                "drpc": { "url": "https://eth.drpc.org" },
                "gateway": { "url": "https://rpc.eth.gateway.fm" }
            }]
        }])
    }

    fn bridges_fixture() -> Value {
        json!([{
            "bridge_id": 1,
            "name": "AMB",
            "type": "amb",
            "enabled": true,
            "api_url": "https://api.example",
            "contracts": [{
                "chain_id": 100,
                "address": "0xf6A78083ca3e2a662D6dd1703c939c8aCE2e268d",
                "version": 6,
                "started_at_block": 10
            }]
        }])
    }

    fn apply(
        root: &mut Value,
        prefix: &str,
        vars: &[(&str, &str)],
        rules: &ArrayRules,
    ) -> Result<Vec<AppliedOverride>> {
        apply_env_overrides(
            root,
            prefix,
            vars.iter().map(|(k, v)| (k.to_string(), v.to_string())),
            rules,
        )
    }

    #[test]
    fn test_apply_scalar_override_on_existing_chain_replaces_field() {
        let mut root = chains_fixture();
        let applied = apply(
            &mut root,
            CHAINS,
            &[("INTERCHAIN_INDEXER_CHAINS__1__NAME", "Mainnet")],
            &CHAINS_RULES,
        )
        .unwrap();

        assert_eq!(root[0]["name"], json!("Mainnet"));
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].json_path, "[chain_id=1].name");
    }

    #[test]
    fn test_apply_new_chain_field_by_field_creates_containers() {
        let mut root = chains_fixture();
        apply(
            &mut root,
            CHAINS,
            &[
                ("INTERCHAIN_INDEXER_CHAINS__137__NAME", "Polygon"),
                (
                    "INTERCHAIN_INDEXER_CHAINS__137__ICON",
                    "https://icon.example/poly.svg",
                ),
                (
                    "INTERCHAIN_INDEXER_CHAINS__137__RPCS__MYNODE__URL",
                    "https://my.node",
                ),
            ],
            &CHAINS_RULES,
        )
        .unwrap();

        assert_eq!(root.as_array().unwrap().len(), 2);
        let chain = &root[1];
        // Injected as a JSON number, not a string.
        assert_eq!(chain["chain_id"], json!(137));
        assert_eq!(chain["name"], json!("Polygon"));
        assert_eq!(chain["icon"], json!("https://icon.example/poly.svg"));
        assert_eq!(
            chain["rpcs"],
            json!([{"mynode": {"url": "https://my.node"}}])
        );
    }

    #[test]
    fn test_apply_new_chain_as_single_fragment_injects_id() {
        let mut root = chains_fixture();
        apply(
            &mut root,
            CHAINS,
            &[(
                "INTERCHAIN_INDEXER_CHAINS__137",
                r#"{"name":"Polygon","icon":"i","rpcs":[{"mynode":{"url":"u"}}]}"#,
            )],
            &CHAINS_RULES,
        )
        .unwrap();

        assert_eq!(root.as_array().unwrap().len(), 2);
        assert_eq!(
            root[1],
            json!({
                "chain_id": 137,
                "name": "Polygon",
                "icon": "i",
                "rpcs": [{"mynode": {"url": "u"}}]
            })
        );
    }

    #[test]
    fn test_apply_fragment_and_deeper_field_var_field_wins() {
        let mut root = chains_fixture();
        apply(
            &mut root,
            CHAINS,
            &[
                // Deliberately listed before the shallower fragment: depth
                // ordering, not iteration order, decides precedence.
                ("INTERCHAIN_INDEXER_CHAINS__137__NAME", "Deep"),
                (
                    "INTERCHAIN_INDEXER_CHAINS__137",
                    r#"{"name":"Fragment","icon":"i"}"#,
                ),
            ],
            &CHAINS_RULES,
        )
        .unwrap();

        assert_eq!(root[1]["name"], json!("Deep"));
        assert_eq!(root[1]["icon"], json!("i"));
    }

    #[test]
    fn test_apply_root_bulk_patch_upserts_by_id() {
        let mut root = chains_fixture();
        let applied = apply(
            &mut root,
            CHAINS,
            &[(
                CHAINS,
                r#"[{"chain_id":1,"name":"Renamed"},{"chain_id":137,"name":"Polygon"}]"#,
            )],
            &CHAINS_RULES,
        )
        .unwrap();

        assert_eq!(applied[0].json_path, "$");
        let chains = root.as_array().unwrap();
        assert_eq!(chains.len(), 2);
        // Merged into the existing element: icon kept, name replaced.
        assert_eq!(chains[0]["name"], json!("Renamed"));
        assert_eq!(chains[0]["icon"], json!("https://icon.example/eth.svg"));
        // Appended as-is.
        assert_eq!(chains[1], json!({"chain_id": 137, "name": "Polygon"}));
    }

    #[test]
    fn test_apply_root_bulk_patch_element_without_id_errors() {
        let mut root = chains_fixture();
        let err = apply(
            &mut root,
            CHAINS,
            &[(CHAINS, r#"[{"name":"NoId"}]"#)],
            &CHAINS_RULES,
        )
        .unwrap_err();

        assert!(err.to_string().contains("chain_id"), "unexpected: {err:#}");
    }

    #[test]
    fn test_apply_null_at_entry_key_path_errors() {
        for chain_id in ["1", "137"] {
            let mut root = chains_fixture();
            let var = format!("INTERCHAIN_INDEXER_CHAINS__{chain_id}");
            let err = apply(&mut root, CHAINS, &[(&var, "null")], &CHAINS_RULES).unwrap_err();
            assert!(
                err.to_string().contains("deletion is not supported"),
                "unexpected: {err:#}"
            );
        }
    }

    #[test]
    fn test_apply_null_field_replaces_value_and_keeps_key() {
        let mut root = bridges_fixture();
        apply(
            &mut root,
            BRIDGES,
            &[("INTERCHAIN_INDEXER_BRIDGES__1__API_URL", "null")],
            &BRIDGES_RULES,
        )
        .unwrap();

        let bridge = root[0].as_object().unwrap();
        assert!(bridge.contains_key("api_url"), "key must be kept");
        assert_eq!(bridge["api_url"], Value::Null);
    }

    #[test]
    fn test_apply_contracts_tune_existing_with_mixed_case_address() {
        let mut root = bridges_fixture();
        let applied = apply(
            &mut root,
            BRIDGES,
            &[(
                "INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__100__0XF6A78083CA3E2A662D6DD1703C939C8ACE2E268D__6__STARTED_AT_BLOCK",
                "99",
            )],
            &BRIDGES_RULES,
        )
        .unwrap();

        let contracts = root[0]["contracts"].as_array().unwrap();
        assert_eq!(contracts.len(), 1, "must match, not append");
        assert_eq!(contracts[0]["started_at_block"], json!(99));
        // The file's mixed-case address is untouched on a match.
        assert_eq!(
            contracts[0]["address"],
            json!("0xf6A78083ca3e2a662D6dd1703c939c8aCE2e268d")
        );
        assert_eq!(
            applied[0].json_path,
            "[bridge_id=1].contracts[chain_id=100,address=0xf6a78083ca3e2a662d6dd1703c939c8ace2e268d,version=6].started_at_block"
        );
    }

    #[test]
    fn test_apply_contracts_new_version_appends_with_injected_keys() {
        let mut root = bridges_fixture();
        apply(
            &mut root,
            BRIDGES,
            &[(
                "INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__100__0xF6A78083CA3E2A662D6DD1703C939C8ACE2E268D__8__STARTED_AT_BLOCK",
                "500",
            )],
            &BRIDGES_RULES,
        )
        .unwrap();

        let contracts = root[0]["contracts"].as_array().unwrap();
        assert_eq!(contracts.len(), 2, "new version must append");
        assert_eq!(
            contracts[1],
            json!({
                "chain_id": 100,
                "address": "0xf6a78083ca3e2a662d6dd1703c939c8ace2e268d",
                "version": 8,
                "started_at_block": 500
            })
        );
    }

    #[test]
    fn test_apply_rpcs_tune_existing_provider() {
        let mut root = chains_fixture();
        let applied = apply(
            &mut root,
            CHAINS,
            &[("INTERCHAIN_INDEXER_CHAINS__1__RPCS__DRPC__MAX_RPS", "5")],
            &CHAINS_RULES,
        )
        .unwrap();

        assert_eq!(root[0]["rpcs"][0]["drpc"]["max_rps"], json!(5));
        assert_eq!(applied[0].json_path, "[chain_id=1].rpcs[drpc].max_rps");
    }

    #[test]
    fn test_apply_rpcs_add_provider_to_existing_map() {
        let mut root = chains_fixture();
        apply(
            &mut root,
            CHAINS,
            &[(
                "INTERCHAIN_INDEXER_CHAINS__1__RPCS__MYNODE",
                r#"{"url":"https://my.node","max_rps":2}"#,
            )],
            &CHAINS_RULES,
        )
        .unwrap();

        let rpcs = root[0]["rpcs"].as_array().unwrap();
        assert_eq!(rpcs.len(), 1, "inserted into the first map, not appended");
        assert_eq!(
            rpcs[0]["mynode"],
            json!({"url": "https://my.node", "max_rps": 2})
        );
        // Existing providers untouched.
        assert_eq!(rpcs[0]["drpc"], json!({"url": "https://eth.drpc.org"}));
    }

    #[test]
    fn test_apply_rpcs_whole_array_replace() {
        let mut root = chains_fixture();
        apply(
            &mut root,
            CHAINS,
            &[(
                "INTERCHAIN_INDEXER_CHAINS__1__RPCS",
                r#"[{"solo":{"url":"https://solo.node"}}]"#,
            )],
            &CHAINS_RULES,
        )
        .unwrap();

        assert_eq!(
            root[0]["rpcs"],
            json!([{"solo": {"url": "https://solo.node"}}])
        );
    }

    #[test]
    fn test_apply_duplicate_normalized_path_errors_naming_both_vars() {
        let mut root = chains_fixture();
        let err = apply(
            &mut root,
            CHAINS,
            &[
                ("INTERCHAIN_INDEXER_CHAINS__1__NAME", "A"),
                ("INTERCHAIN_INDEXER_CHAINS__1__Name", "B"),
            ],
            &CHAINS_RULES,
        )
        .unwrap_err();

        let message = err.to_string();
        assert!(
            message.contains("INTERCHAIN_INDEXER_CHAINS__1__NAME")
                && message.contains("INTERCHAIN_INDEXER_CHAINS__1__Name"),
            "unexpected: {message}"
        );
    }

    #[test]
    fn test_apply_value_parsing_json_first_string_fallback() {
        let mut root = chains_fixture();
        apply(
            &mut root,
            CHAINS,
            &[
                ("INTERCHAIN_INDEXER_CHAINS__1__RPCS__DRPC__ENABLED", "true"),
                ("INTERCHAIN_INDEXER_CHAINS__1__RPCS__DRPC__MAX_RPS", "123"),
                (
                    "INTERCHAIN_INDEXER_CHAINS__1__RPCS__DRPC__URL",
                    "https://new.drpc.org",
                ),
                ("INTERCHAIN_INDEXER_CHAINS__1__ICON", "0xABCDEF"),
                // A literal string that is valid JSON needs JSON-string quoting.
                ("INTERCHAIN_INDEXER_CHAINS__1__NAME", "\"123\""),
            ],
            &CHAINS_RULES,
        )
        .unwrap();

        let drpc = &root[0]["rpcs"][0]["drpc"];
        assert_eq!(drpc["enabled"], json!(true));
        assert_eq!(drpc["max_rps"], json!(123));
        assert_eq!(drpc["url"], json!("https://new.drpc.org"));
        assert_eq!(root[0]["icon"], json!("0xABCDEF"));
        assert_eq!(root[0]["name"], json!("123"));
    }

    #[test]
    fn test_apply_fragment_with_conflicting_id_errors() {
        // Both against an existing entry and a to-be-created one.
        for chain_id in ["1", "137"] {
            let mut root = chains_fixture();
            let var = format!("INTERCHAIN_INDEXER_CHAINS__{chain_id}");
            let err = apply(
                &mut root,
                CHAINS,
                &[(&var, r#"{"chain_id":2,"name":"Retargeted"}"#)],
                &CHAINS_RULES,
            )
            .unwrap_err();
            assert!(
                err.to_string()
                    .contains("conflicts with the addressed entry"),
                "unexpected: {err:#}"
            );
        }
    }

    #[test]
    fn test_apply_fragment_with_matching_id_is_allowed() {
        let mut root = chains_fixture();
        apply(
            &mut root,
            CHAINS,
            &[(
                "INTERCHAIN_INDEXER_CHAINS__137",
                r#"{"chain_id":137,"name":"Polygon"}"#,
            )],
            &CHAINS_RULES,
        )
        .unwrap();

        assert_eq!(root[1]["chain_id"], json!(137));
        assert_eq!(root[1]["name"], json!("Polygon"));
    }

    #[test]
    fn test_apply_contract_fragment_with_conflicting_key_field_errors() {
        let mut root = bridges_fixture();
        let err = apply(
            &mut root,
            BRIDGES,
            &[(
                "INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__100__0xf6A78083ca3e2a662D6dd1703c939c8aCE2e268d__6",
                r#"{"version":8,"started_at_block":99}"#,
            )],
            &BRIDGES_RULES,
        )
        .unwrap_err();

        assert!(err.to_string().contains("'version'"), "unexpected: {err:#}");
    }

    #[test]
    fn test_apply_direct_key_field_var_conflicting_value_errors() {
        let mut root = chains_fixture();
        let err = apply(
            &mut root,
            CHAINS,
            &[("INTERCHAIN_INDEXER_CHAINS__137__CHAIN_ID", "1")],
            &CHAINS_RULES,
        )
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("conflicts with the addressed entry"),
            "unexpected: {err:#}"
        );

        // A redundant-but-consistent id set is allowed.
        let mut root = chains_fixture();
        apply(
            &mut root,
            CHAINS,
            &[
                ("INTERCHAIN_INDEXER_CHAINS__137__CHAIN_ID", "137"),
                ("INTERCHAIN_INDEXER_CHAINS__137__NAME", "Polygon"),
            ],
            &CHAINS_RULES,
        )
        .unwrap();
        assert_eq!(root[1]["chain_id"], json!(137));
    }

    #[test]
    fn test_apply_descending_into_key_field_errors() {
        let mut root = chains_fixture();
        let err = apply(
            &mut root,
            CHAINS,
            &[("INTERCHAIN_INDEXER_CHAINS__1__CHAIN_ID__FOO", "1")],
            &CHAINS_RULES,
        )
        .unwrap_err();

        assert!(
            err.to_string().contains("cannot descend into key field"),
            "unexpected: {err:#}"
        );
    }

    #[test]
    fn test_apply_non_object_entry_value_errors() {
        let mut root = chains_fixture();
        let err = apply(
            &mut root,
            CHAINS,
            &[("INTERCHAIN_INDEXER_CHAINS__137", "42")],
            &CHAINS_RULES,
        )
        .unwrap_err();

        assert!(
            err.to_string().contains("must be a JSON object"),
            "unexpected: {err:#}"
        );
    }

    #[test]
    fn test_apply_unrelated_vars_are_ignored() {
        let mut root = chains_fixture();
        let before = root.clone();
        let applied = apply(
            &mut root,
            CHAINS,
            &[
                ("INTERCHAIN_INDEXER__CHAINS_CONFIG", "config/chains.json"),
                ("INTERCHAIN_INDEXER_CHAINSFOO__1__NAME", "X"),
                ("PATH", "/usr/bin"),
            ],
            &CHAINS_RULES,
        )
        .unwrap();

        assert!(applied.is_empty());
        assert_eq!(root, before);
    }
}
