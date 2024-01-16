use std::str::FromStr;

use anyhow::Context;
use ethers::types::Address;

use super::{DomainToken, DomainTokenType, SubgraphSettings};
use crate::entity::subgraph::domain::DetailedDomain;

#[tracing::instrument(
    level = "info",
    skip(domain, subgraph_settings),
    fields(domain_name = domain.name),
    err,
)]
pub fn extract_tokens_from_domain(
    domain: &DetailedDomain,
    subgraph_settings: &SubgraphSettings,
) -> Result<Vec<DomainToken>, anyhow::Error> {
    let mut tokens = vec![];

    if let Some(contract) = subgraph_settings.native_token_contract {
        let is_second_level_domain = domain
            .name
            .as_ref()
            .map(|name| name.matches('.').count() == 1)
            .unwrap_or(true);
        // native NFT exists only if domain is second level (like abc.eth and not abc.abc.eth)
        if is_second_level_domain {
            let labelhash = domain
                .labelhash
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("no labelhash in database"))?;
            let id = hex::encode(labelhash);
            tokens.push(DomainToken {
                id,
                contract,
                _type: DomainTokenType::Native,
            });
        }
    };

    if domain.wrapped_owner.is_some() {
        let id = domain.id.clone();
        let contract = Address::from_str(&domain.owner).context("parse owner as address")?;
        tokens.push(DomainToken {
            id,
            contract,
            _type: DomainTokenType::Wrapped,
        });
    };

    Ok(tokens)
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     fn domain() -> DetailedDomain {
//         todo!()
//     }

//     fn addr(a: &str) -> Option<Address> {
//         Address::from_str(a).ok()
//     }

//     #[test]
//     fn it_works() {
//         for (domain, native_token_contract, expected_tokens) in [
//             (domain(), addr(""), vec![
//                 DomainToken {

//                 }
//             ])
//         ]
//     }
// }
