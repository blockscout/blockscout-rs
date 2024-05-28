use anyhow::Context;
use config::{Config, File, FileFormat};
use json_dotpath::DotPaths;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    marker::PhantomData,
    ops::Not,
    path::{Path, PathBuf},
};

const ANCHOR_START: &str = "anchors.envs.start";
const ANCHOR_END: &str = "anchors.envs.end";

pub fn run_env_collector_cli<S: Serialize + DeserializeOwned>(
    service_name: &str,
    markdown_path: &str,
    config_path: &str,
    skip_vars: &[&str],
) {
    let collector = EnvCollector::<S>::new(
        service_name.to_string(),
        markdown_path.into(),
        config_path.into(),
        skip_vars.iter().map(|s| s.to_string()).collect(),
    );
    let validate_only = std::env::var("VALIDATE_ONLY")
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
    skip_vars: Vec<String>,

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
        skip_vars: Vec<String>,
    ) -> Self {
        Self {
            service_name,
            markdown_path,
            config_path,
            skip_vars,
            settings: Default::default(),
        }
    }

    pub fn find_missing(&self) -> Result<Vec<EnvVariable>, anyhow::Error> {
        find_missing_variables_in_markdown::<S>(
            &self.service_name,
            self.markdown_path.as_path(),
            self.config_path.as_path(),
            self.skip_vars.clone(),
        )
    }

    pub fn update_markdown(&self) -> Result<(), anyhow::Error> {
        update_markdown_file::<S>(
            &self.service_name,
            self.markdown_path.as_path(),
            self.config_path.as_path(),
            self.skip_vars.clone(),
        )
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq)]
pub struct EnvVariable {
    pub key: String,
    pub description: String,
    pub required: bool,
    pub default_value: Option<String>,
}

impl PartialEq<Self> for EnvVariable {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.required == other.required
            && self.default_value == other.default_value
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
    pub fn from_example_toml<S>(
        service_prefix: &str,
        example_toml_config_content: &str,
        skip_vars: Vec<String>,
    ) -> Result<Self, anyhow::Error>
    where
        S: Serialize + DeserializeOwned,
    {
        let settings: S = Config::builder()
            .add_source(File::from_str(
                example_toml_config_content,
                FileFormat::Toml,
            ))
            .build()
            .context("failed to build config")?
            .try_deserialize()
            .context("failed to deserialize config")?;
        let json = serde_json::to_value(&settings).context("failed to convert config to json")?;
        let from_config: Envs = flatten_json(&json, service_prefix)
            .into_iter()
            .filter(|(key, _)| !skip_vars.iter().any(|s| key.starts_with(s)))
            .map(|(key, value)| {
                let required =
                    var_is_required(&settings, &from_key_to_json_path(&key, service_prefix));
                let default_value = required.not().then_some(value);
                let var = EnvVariable {
                    key: key.clone(),
                    required,
                    default_value,
                    description: Default::default(),
                };

                (key, var)
            })
            .collect::<BTreeMap<_, _>>()
            .into();

        Ok(from_config)
    }

    pub fn from_markdown(markdown_content: &str) -> Result<Self, anyhow::Error> {
        let line_start = markdown_content
            .find(ANCHOR_START)
            .context("anchors.envs.start not found")?
            + ANCHOR_START.len();
        let line_end = markdown_content
            .find(ANCHOR_END)
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
    skip_vars: Vec<String>,
) -> Result<Vec<EnvVariable>, anyhow::Error>
where
    S: Serialize + DeserializeOwned,
{
    let example = Envs::from_example_toml::<S>(
        service_name,
        std::fs::read_to_string(config_path)
            .context("failed to read example file")?
            .as_str(),
        skip_vars,
    )?;
    let markdown: Envs = Envs::from_markdown(
        std::fs::read_to_string(markdown_path)
            .context("failed to read markdown file")?
            .as_str(),
    )?;

    let missing = example
        .vars
        .iter()
        .filter(|(key, value)| {
            let maybe_markdown_var = markdown.vars.get(*key);
            maybe_markdown_var.map(|var| var != *value).unwrap_or(true)
        })
        .map(|(_, value)| value.clone())
        .collect();

    Ok(missing)
}

fn update_markdown_file<S>(
    service_name: &str,
    markdown_path: &Path,
    config_path: &Path,
    skip_vars: Vec<String>,
) -> Result<(), anyhow::Error>
where
    S: Serialize + DeserializeOwned,
{
    let from_config = Envs::from_example_toml::<S>(
        service_name,
        std::fs::read_to_string(config_path)
            .context("failed to read config file")?
            .as_str(),
        skip_vars,
    )?;
    let mut markdown_config = Envs::from_markdown(
        std::fs::read_to_string(markdown_path)
            .context("failed to read markdown file")?
            .as_str(),
    )?;
    markdown_config.update_no_override(from_config);
    let table = serialize_env_vars_to_md_table(markdown_config);

    let content = std::fs::read_to_string(markdown_path).context("failed to read markdown file")?;
    let lines = content.lines().collect::<Vec<&str>>();
    let line_start = lines
        .iter()
        .position(|line| line.contains(ANCHOR_START))
        .context("anchors.envs.start not found in markdown")?;
    let line_end = lines
        .iter()
        .position(|line| line.contains(ANCHOR_END))
        .context("anchors.envs.end not found in markdown")?;

    let new_content = [&lines[..=line_start], &[&table], &lines[line_end..]].concat();
    std::fs::write(markdown_path, new_content.join("\n")).context("failed to write file")?;
    Ok(())
}

fn var_is_required<S>(settings: &S, path: &str) -> bool
where
    S: Serialize + DeserializeOwned,
{
    let mut json = serde_json::to_value(settings).unwrap();
    json.dot_remove(path).unwrap();
    let result = serde_json::from_value::<S>(json);
    result.is_err()
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
                Value::String(s) => s.to_string(),
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

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    struct TestSettings {
        pub test: String,
        #[serde(default)]
        pub test2: i32,
        pub database: DatabaseSettings,
    }

    fn var(key: &str, val: Option<&str>, required: bool) -> (String, EnvVariable) {
        (
            key.into(),
            EnvVariable {
                key: key.to_string(),
                default_value: val.map(str::to_string),
                required,
                description: "".to_string(),
            },
        )
    }

    fn default_example() -> &'static str {
        r#"test = "value"
test2 = 123
[database.connect]
url = "test-url"
"#
    }

    fn default_envs() -> Envs {
        Envs::from(BTreeMap::from_iter(vec![
            var("TEST_SERVICE__TEST", None, true),
            var(
                "TEST_SERVICE__DATABASE__CREATE_DATABASE",
                Some("false"),
                false,
            ),
            var(
                "TEST_SERVICE__DATABASE__RUN_MIGRATIONS",
                Some("false"),
                false,
            ),
            var("TEST_SERVICE__TEST2", Some("123"), false),
            var("TEST_SERVICE__DATABASE__CONNECT__URL", None, true),
        ]))
    }

    fn default_markdown() -> &'static str {
        r#"
[anchor]: <> (anchors.envs.start)

| Variable                                  | Required    | Description | Default Value |
|-------------------------------------------|-------------|-------------|---------------|
| `TEST_SERVICE__TEST`                      | true        |             |               |
| `TEST_SERVICE__DATABASE__CREATE_DATABASE` | false       |             | `false`       |
| `TEST_SERVICE__DATABASE__RUN_MIGRATIONS`  | false       |             | `false`       |
| `TEST_SERVICE__TEST2`                     | false       |             | `123`         |
| `TEST_SERVICE__DATABASE__CONNECT__URL`    | true        |             |               |
[anchor]: <> (anchors.envs.end)
"#
    }

    #[test]
    fn from_example_works() {
        let vars =
            Envs::from_example_toml::<TestSettings>("TEST_SERVICE", default_example(), vec![])
                .unwrap();
        let expected = default_envs();
        assert_eq!(vars, expected);
    }

    #[test]
    fn from_markdown_works() {
        let markdown = default_markdown();
        let vars = Envs::from_markdown(markdown).unwrap();
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

        let mut config = tempfile::NamedTempFile::new().unwrap();
        writeln!(config, "{}", default_example()).unwrap();

        let collector = EnvCollector::<TestSettings>::new(
            "TEST_SERVICE".to_string(),
            markdown.path().to_path_buf(),
            config.path().to_path_buf(),
            vec![],
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
| `TEST_SERVICE__DATABASE__CONNECT__URL` | true | | |
| `TEST_SERVICE__TEST` | true | | |
| `SOME_EXTRA_VARS` | | comment should be saved. `kek` | `example_value` |
| `TEST_SERVICE__DATABASE__CREATE_DATABASE` | | | `false` |
| `TEST_SERVICE__DATABASE__RUN_MIGRATIONS` | | | `false` |
| `TEST_SERVICE__TEST2` | | | `123` |

[anchor]: <> (anchors.envs.end)
"#
        );
    }
}
