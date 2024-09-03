use anyhow::Context;
use config::{Config, File};
use json_dotpath::DotPaths;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    marker::PhantomData,
    path::{Path, PathBuf},
};

const ANCHOR_START: &str = "anchors.envs.start";
const ANCHOR_END: &str = "anchors.envs.end";
const VALIDATE_ONLY_ENV: &str = "VALIDATE_ONLY";

pub fn run_env_collector_cli<S: Serialize + DeserializeOwned>(
    service_name: &str,
    markdown_path: &str,
    config_path: &str,
    vars_filter: PrefixFilter,
    anchor_postfix: Option<&str>,
) {
    let collector = EnvCollector::<S>::new(
        service_name.to_string(),
        markdown_path.into(),
        config_path.into(),
        vars_filter,
        anchor_postfix.map(|s| s.to_string()),
    );
    let validate_only = std::env::var(VALIDATE_ONLY_ENV)
        .unwrap_or_default()
        .to_lowercase()
        .eq("true");
    let missing = collector
        .find_missing()
        .expect("Failed to find missing variables");
    if missing.is_empty() {
        println!("All variables are documented");
    } else {
        println!("Found missing variables:");
        for env in missing {
            println!("  {}", env.key);
        }

        if validate_only {
            std::process::exit(1);
        } else {
            println!("Ready to update markdown file: {}", markdown_path);
            println!("Press any key to continue...");
            std::io::stdin().read_line(&mut String::new()).unwrap();
            collector
                .update_markdown()
                .expect("Failed to update markdown");
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnvCollector<S> {
    service_name: String,
    markdown_path: PathBuf,
    config_path: PathBuf,
    vars_filter: PrefixFilter,
    anchor_postfix: Option<String>,

    settings: PhantomData<S>,
}

impl<S> EnvCollector<S>
where
    S: Serialize + DeserializeOwned,
{
    pub fn new(
        service_name: String,
        markdown_path: PathBuf,
        config_path: PathBuf,
        vars_filter: PrefixFilter,
        anchor_postfix: Option<String>,
    ) -> Self {
        Self {
            service_name,
            markdown_path,
            config_path,
            vars_filter,
            anchor_postfix,
            settings: Default::default(),
        }
    }

    pub fn find_missing(&self) -> Result<Vec<EnvVariable>, anyhow::Error> {
        find_missing_variables_in_markdown::<S>(
            &self.service_name,
            self.markdown_path.as_path(),
            self.config_path.as_path(),
            self.vars_filter.clone(),
            self.anchor_postfix.clone(),
        )
    }

    pub fn update_markdown(&self) -> Result<(), anyhow::Error> {
        update_markdown_file::<S>(
            &self.service_name,
            self.markdown_path.as_path(),
            self.config_path.as_path(),
            self.vars_filter.clone(),
            self.anchor_postfix.clone(),
        )
    }
}

#[derive(Debug, Clone)]
pub enum PrefixFilter {
    Whitelist(Vec<String>),
    Blacklist(Vec<String>),
    Empty,
}

impl PrefixFilter {
    pub fn whitelist(allow_only: &[&str]) -> Self {
        let list = allow_only.iter().map(|s| s.to_string()).collect();
        Self::Whitelist(list)
    }

    pub fn blacklist(vars_filter: &[&str]) -> Self {
        let list = vars_filter.iter().map(|s| s.to_string()).collect();
        Self::Blacklist(list)
    }

    pub fn filter(&self, string: &str) -> bool {
        let list = match self {
            PrefixFilter::Whitelist(list) | PrefixFilter::Blacklist(list) => list,
            PrefixFilter::Empty => &vec![],
        };
        let input_matches_some_prefix = list.iter().any(|prefix| string.starts_with(prefix));
        match self {
            PrefixFilter::Whitelist(_) => input_matches_some_prefix,
            PrefixFilter::Blacklist(_) => !input_matches_some_prefix,
            PrefixFilter::Empty => true,
        }
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, PartialEq, Eq)]
pub struct EnvVariable {
    pub key: String,
    pub description: String,
    pub required: bool,
    pub default_value: Option<String>,
}

impl EnvVariable {
    pub fn eq_with_ignores(&self, other: &Self) -> bool {
        self.key == other.key && self.required == other.required
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
struct Envs {
    vars: BTreeMap<String, EnvVariable>,
}

impl From<BTreeMap<String, EnvVariable>> for Envs {
    fn from(vars: BTreeMap<String, EnvVariable>) -> Self {
        Self { vars }
    }
}

impl Envs {
    pub fn from_example<S>(
        service_prefix: &str,
        example_config_path: &str,
        vars_filter: PrefixFilter,
    ) -> Result<Self, anyhow::Error>
    where
        S: Serialize + DeserializeOwned,
    {
        let settings: S = Config::builder()
            .add_source(File::with_name(example_config_path))
            .build()
            .context("failed to build config")?
            .try_deserialize()
            .context("failed to deserialize config")?;
        let json = serde_json::to_value(&settings).context("failed to convert config to json")?;
        let from_config: Envs = flatten_json(&json, service_prefix)
            .into_iter()
            .filter(|(key, _)| vars_filter.filter(&key))
            .map(|(key, value)| {
                let default_value =
                    default_of_var(&settings, &from_key_to_json_path(&key, service_prefix));
                let required = default_value.is_none();
                let description = try_get_description(&key, &value, &default_value);
                let default_value = default_value.map(|v| v.to_string());
                let var = EnvVariable {
                    key: key.clone(),
                    required,
                    default_value,
                    description,
                };

                (key, var)
            })
            .collect::<BTreeMap<_, _>>()
            .into();

        Ok(from_config)
    }

    pub fn from_markdown(
        markdown_content: &str,
        anchor_postfix: Option<String>,
    ) -> Result<Self, anyhow::Error> {
        let start_anchor = push_postfix_to_anchor(ANCHOR_START, anchor_postfix.clone());
        let line_start = markdown_content
            .find(&start_anchor)
            .context("anchors.envs.start not found")?
            + start_anchor.len();
        let end_anchor = push_postfix_to_anchor(ANCHOR_END, anchor_postfix);
        let line_end = markdown_content
            .find(&end_anchor)
            .context("anchors.envs.end not found")?
            - 1;
        let table_content = &markdown_content[line_start..=line_end];

        let re = regex::Regex::new(regex_md_table_row()).context("regex creation")?;
        let result = re
            .captures_iter(table_content)
            .map(|c| c.extract())
            .map(|(_, [key, required, description, default_value])| {
                let required = required.trim().eq("true");
                let default_value = if default_value.trim().is_empty() {
                    None
                } else {
                    Some(default_value.to_string())
                };
                let var = EnvVariable {
                    key: key.to_string(),
                    default_value,
                    required,
                    description: description.trim().to_string(),
                };

                (key.to_string(), var)
            })
            .collect::<BTreeMap<_, _>>()
            .into();

        Ok(result)
    }

    pub fn update_no_override(&mut self, other: Envs) {
        for (key, value) in other.vars {
            self.vars.entry(key).or_insert(value);
        }
    }

    pub fn sorted_with_required(&self) -> impl IntoIterator<Item = (&String, &EnvVariable)> {
        let mut vars = self.vars.iter().collect::<Vec<_>>();
        vars.sort_by_key(|(k, v)| (!v.required, *k));
        vars
    }
}

fn find_missing_variables_in_markdown<S>(
    service_name: &str,
    markdown_path: &Path,
    config_path: &Path,
    vars_filter: PrefixFilter,
    anchor_postfix: Option<String>,
) -> Result<Vec<EnvVariable>, anyhow::Error>
where
    S: Serialize + DeserializeOwned,
{
    let example = Envs::from_example::<S>(
        service_name,
        config_path
            .to_str()
            .expect("config path is not valid utf-8"),
        vars_filter,
    )?;
    let markdown: Envs = Envs::from_markdown(
        std::fs::read_to_string(markdown_path)
            .context("failed to read markdown file")?
            .as_str(),
        anchor_postfix,
    )?;

    let missing = example
        .vars
        .iter()
        .filter(|(key, value)| {
            let maybe_markdown_var = markdown.vars.get(*key);
            maybe_markdown_var
                .map(|var| !var.eq_with_ignores(value))
                .unwrap_or(true)
        })
        .map(|(_, value)| value.clone())
        .collect();

    Ok(missing)
}

fn update_markdown_file<S>(
    service_name: &str,
    markdown_path: &Path,
    config_path: &Path,
    vars_filter: PrefixFilter,
    anchor_postfix: Option<String>,
) -> Result<(), anyhow::Error>
where
    S: Serialize + DeserializeOwned,
{
    let from_config = Envs::from_example::<S>(
        service_name,
        config_path
            .to_str()
            .expect("config path is not valid utf-8"),
        vars_filter,
    )?;
    let mut markdown_config = Envs::from_markdown(
        std::fs::read_to_string(markdown_path)
            .context("failed to read markdown file")?
            .as_str(),
        anchor_postfix.clone(),
    )?;
    markdown_config.update_no_override(from_config);
    let table = serialize_env_vars_to_md_table(markdown_config);

    let content = std::fs::read_to_string(markdown_path).context("failed to read markdown file")?;
    let lines = content.lines().collect::<Vec<&str>>();
    let line_start = lines
        .iter()
        .position(|line| {
            line.contains(&push_postfix_to_anchor(
                ANCHOR_START,
                anchor_postfix.clone(),
            ))
        })
        .context("anchors.envs.start not found in markdown")?;
    let line_end = lines
        .iter()
        .position(|line| line.contains(&push_postfix_to_anchor(ANCHOR_END, anchor_postfix.clone())))
        .context("anchors.envs.end not found in markdown")?;

    let new_content = [&lines[..=line_start], &[&table], &lines[line_end..]].concat();
    std::fs::write(markdown_path, new_content.join("\n")).context("failed to write file")?;
    Ok(())
}

fn default_of_var<S>(settings: &S, path: &str) -> Option<serde_json::Value>
where
    S: Serialize + DeserializeOwned,
{
    let mut json = serde_json::to_value(settings).expect("structure should be serializable");
    json.dot_remove(path).expect("value path not found");

    let settings_with_default_value = serde_json::from_value::<S>(json).ok()?;
    let json: serde_json::Value = serde_json::to_value(&settings_with_default_value)
        .expect("structure should be serializable");
    let default_value: serde_json::Value = json
        .dot_get(path)
        .expect("value path not found")
        .unwrap_or_default();
    Some(default_value)
}

fn try_get_description(_key: &str, value: &str, default: &Option<serde_json::Value>) -> String {
    if value.is_empty() {
        return Default::default();
    }
    let default_str = default.as_ref().map(|v| v.to_string()).unwrap_or_default();

    // If the value is the same as the default value, we don't need to show it in the description
    if default_str == value {
        return Default::default();
    }

    format!("e.g. `{}`", value)
}

fn from_key_to_json_path(key: &str, service_prefix: &str) -> String {
    key.trim_start_matches(&format!("{service_prefix}__"))
        .to_lowercase()
        .replace("__", ".")
        .to_string()
}

fn serialize_env_vars_to_md_table(vars: Envs) -> String {
    let mut result = r#"
| Variable | Required | Description | Default value |
| --- | --- | --- | --- |
"#
    .to_string();

    for (key, env) in vars.sorted_with_required() {
        let required = if env.required { " true " } else { " " };
        let description = if env.description.is_empty() {
            " ".to_string()
        } else {
            format!(" {} ", env.description)
        };
        let default_value = env
            .default_value
            .as_ref()
            .map(|v| format!(" `{v}` "))
            .unwrap_or(" ".to_string());
        result.push_str(&format!(
            "| `{}` |{}|{}|{}|\n",
            key, required, description, default_value,
        ));
    }
    result
}

fn push_postfix_to_anchor(anchor: &str, postfix: Option<String>) -> String {
    if let Some(postfix) = postfix {
        format!("{anchor}.{postfix}")
    } else {
        anchor.to_string()
    }
}

fn regex_md_table_row() -> &'static str {
    concat!(
        r"\|\s*`([^|`]*)`\s*",
        r"\|\s*([^|]*)\s*",
        r"\|\s*([^|]*)\s*",
        r"\|\s*`?([^|`]*)`?\s*",
        r#"\|"#
    )
}

fn flatten_json(json: &Value, initial_prefix: &str) -> BTreeMap<String, String> {
    let mut env_vars = BTreeMap::new();
    _flat_json(json, initial_prefix, &mut env_vars);
    env_vars
}

fn _flat_json(json: &Value, prefix: &str, env_vars: &mut BTreeMap<String, String>) {
    match json {
        Value::Object(map) => {
            for (key, value) in map {
                let new_prefix = if prefix.is_empty() {
                    key.to_string()
                } else {
                    format!("{}__{}", prefix, key)
                };
                _flat_json(value, &new_prefix, env_vars);
            }
        }
        _ => {
            let env_var_name = prefix.to_uppercase();
            let env_var_value = match json {
                Value::String(s) => format!("\"{}\"", s.to_string()),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Null => "null".to_string(),
                _ => panic!("unsupported value type: {:?}", json),
            };
            env_vars.insert(env_var_name, env_var_value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockscout_service_launcher::database::DatabaseSettings;
    use pretty_assertions::assert_eq;
    use serde::Deserialize;
    use std::io::Write;

    #[test]
    fn filter_works() {
        let list = vec!["LOL", "KEK__"];
        let blacklist = PrefixFilter::blacklist(&list);
        assert!(!blacklist.filter("LOL"));
        assert!(!blacklist.filter("LOL_KEK"));
        assert!(blacklist.filter("KEK"));
        assert!(blacklist.filter("KEK_"));
        assert!(!blacklist.filter("KEK__"));
        assert!(!blacklist.filter("KEK__KKEKEKEKEK"));
        assert!(blacklist.filter("hesoyam"));

        let whitelist = PrefixFilter::whitelist(&list);
        assert!(whitelist.filter("LOL"));
        assert!(whitelist.filter("LOL_KEK"));
        assert!(!whitelist.filter("KEK"));
        assert!(!whitelist.filter("KEK_"));
        assert!(whitelist.filter("KEK__"));
        assert!(whitelist.filter("KEK__KKEKEKEKEK"));
        assert!(!whitelist.filter("hesoyam"));
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    struct TestSettings {
        pub test: String,
        #[serde(default = "default_test2")]
        pub test2: i32,
        pub test3_set: Option<bool>,
        pub test4_not_set: Option<bool>,
        #[serde(default = "very_cool_string")]
        pub string_with_default: String,
        pub database: DatabaseSettings,
    }

    fn default_test2() -> i32 {
        1000
    }

    fn very_cool_string() -> String {
        "kekek".into()
    }

    fn var(
        key: &str,
        val: Option<&str>,
        required: bool,
        description: &str,
    ) -> (String, EnvVariable) {
        (
            key.into(),
            EnvVariable {
                key: key.to_string(),
                default_value: val.map(str::to_string),
                required,
                description: description.into(),
            },
        )
    }

    fn tempfile_with_content(content: &str, format: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::Builder::new().suffix(format).tempfile().unwrap();
        writeln!(file, "{}", content).unwrap();
        file
    }

    fn default_config_example_file_toml() -> tempfile::NamedTempFile {
        let content = r#"test = "value"
        test2 = 123
        test3_set = false
        [database.connect]
        url = "test-url"
        "#;
        tempfile_with_content(content, ".toml")
    }

    fn default_config_example_file_json() -> tempfile::NamedTempFile {
        let content = r#"{
            "test": "value",
            "test2": 123,
            "test3_set": false,
            "database": {
                "connect": {
                    "url": "test-url"
                }
            }
        }"#;
        tempfile_with_content(content, ".json")
    }

    fn default_envs() -> Envs {
        Envs::from(BTreeMap::from_iter(vec![
            var("TEST_SERVICE__TEST", None, true, "e.g. `\"value\"`"),
            var(
                "TEST_SERVICE__DATABASE__CREATE_DATABASE",
                Some("false"),
                false,
                "",
            ),
            var(
                "TEST_SERVICE__DATABASE__RUN_MIGRATIONS",
                Some("false"),
                false,
                "",
            ),
            var("TEST_SERVICE__TEST2", Some("1000"), false, "e.g. `123`"),
            var(
                "TEST_SERVICE__TEST3_SET",
                Some("null"),
                false,
                "e.g. `false`",
            ),
            var("TEST_SERVICE__TEST4_NOT_SET", Some("null"), false, ""),
            var(
                "TEST_SERVICE__STRING_WITH_DEFAULT",
                Some("\"kekek\""),
                false,
                "",
            ),
            var(
                "TEST_SERVICE__DATABASE__CONNECT__URL",
                None,
                true,
                "e.g. `\"test-url\"`",
            ),
        ]))
    }

    fn default_markdown_content() -> &'static str {
        r#"

[anchor]: <> (anchors.envs.start.irrelevant_postfix)
[anchor]: <> (anchors.envs.end.irrelevant_postfix)

[anchor]: <> (anchors.envs.start.cool_postfix)

| Variable                                  | Required    | Description      | Default Value |
|-------------------------------------------|-------------|------------------|---------------|
| `TEST_SERVICE__TEST`                      | true        | e.g. `"value"`   |               |
| `TEST_SERVICE__DATABASE__CREATE_DATABASE` | false       |                  | `false`       |
| `TEST_SERVICE__DATABASE__RUN_MIGRATIONS`  | false       |                  | `false`       |
| `TEST_SERVICE__TEST2`                     | false       | e.g. `123`       | `1000`        |
| `TEST_SERVICE__TEST3_SET`                 | false       | e.g. `false`     | `null`        |
| `TEST_SERVICE__TEST4_NOT_SET`             | false       |                  | `null`        |
| `TEST_SERVICE__STRING_WITH_DEFAULT`       | false       |                  | `"kekek"`     |
| `TEST_SERVICE__DATABASE__CONNECT__URL`    | true        | e.g. `"test-url"`|               |
[anchor]: <> (anchors.envs.end.cool_postfix)
"#
    }

    #[test]
    fn from_toml_example_works() {
        let example_file = default_config_example_file_toml();
        let vars = Envs::from_example::<TestSettings>(
            "TEST_SERVICE",
            example_file.path().to_str().unwrap(),
            PrefixFilter::Empty,
        )
        .unwrap();
        let expected = default_envs();
        assert_eq!(vars, expected);
    }

    #[test]
    fn from_json_example_works() {
        let example_file = default_config_example_file_json();
        let vars = Envs::from_example::<TestSettings>(
            "TEST_SERVICE",
            example_file.path().to_str().unwrap(),
            PrefixFilter::Empty,
        )
        .unwrap();
        let expected = default_envs();
        assert_eq!(vars, expected);
    }

    #[test]
    fn from_markdown_works() {
        let markdown = default_markdown_content();
        let vars = Envs::from_markdown(markdown, Some("cool_postfix".to_string())).unwrap();
        let expected = default_envs();
        assert_eq!(vars, expected);
    }

    #[test]
    fn update_and_validate_works() {
        let mut markdown = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            markdown,
            r#"
[anchor]: <> (anchors.envs.start)
|`SOME_EXTRA_VARS`| | comment should be saved. `kek` |`example_value` |
|`SOME_EXTRA_VARS2`| true |        |`example_value2` |

[anchor]: <> (anchors.envs.end)
"#
        )
        .unwrap();

        let config = default_config_example_file_toml();

        let collector = EnvCollector::<TestSettings>::new(
            "TEST_SERVICE".to_string(),
            markdown.path().to_path_buf(),
            config.path().to_path_buf(),
            PrefixFilter::Empty,
            None,
        );

        let missing = collector.find_missing().unwrap();
        assert_eq!(
            missing,
            default_envs()
                .vars
                .values()
                .map(Clone::clone)
                .collect::<Vec<EnvVariable>>()
        );

        collector.update_markdown().unwrap();
        let missing = collector.find_missing().unwrap();
        assert_eq!(missing, vec![]);

        let markdown_content = std::fs::read_to_string(markdown.path()).unwrap();
        assert_eq!(
            markdown_content,
            r#"
[anchor]: <> (anchors.envs.start)

| Variable | Required | Description | Default value |
| --- | --- | --- | --- |
| `SOME_EXTRA_VARS2` | true | | `example_value2` |
| `TEST_SERVICE__DATABASE__CONNECT__URL` | true | e.g. `"test-url"` | |
| `TEST_SERVICE__TEST` | true | e.g. `"value"` | |
| `SOME_EXTRA_VARS` | | comment should be saved. `kek` | `example_value` |
| `TEST_SERVICE__DATABASE__CREATE_DATABASE` | | | `false` |
| `TEST_SERVICE__DATABASE__RUN_MIGRATIONS` | | | `false` |
| `TEST_SERVICE__STRING_WITH_DEFAULT` | | | `"kekek"` |
| `TEST_SERVICE__TEST2` | | e.g. `123` | `1000` |
| `TEST_SERVICE__TEST3_SET` | | e.g. `false` | `null` |
| `TEST_SERVICE__TEST4_NOT_SET` | | | `null` |

[anchor]: <> (anchors.envs.end)
"#
        );
    }
}
