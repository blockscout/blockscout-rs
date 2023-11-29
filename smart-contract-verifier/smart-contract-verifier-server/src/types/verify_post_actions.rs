use std::{collections::HashSet, str::FromStr};

#[derive(PartialEq, Eq, Hash)]
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
pub fn parse_post_actions(actions: &[String]) -> anyhow::Result<HashSet<VerifyPostAction>> {
    actions.iter().map(|action| action.parse()).collect()
}
