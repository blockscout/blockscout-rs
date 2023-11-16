use std::{collections::HashSet, str::FromStr};

pub enum VerifyPostAction {
    LookupMethods,
}

impl FromStr for VerifyPostAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "lookup-methods" => Ok(VerifyPostAction::LookupMethods),
            _ => Err(anyhow::anyhow!("invalid post action {s}")),
        }
    }
}
pub fn parse_post_actions(actions: &[String]) -> anyhow::Result<Vec<VerifyPostAction>> {
    actions
        .iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|action| action.parse())
        .collect()
}
