// SPDX-License-Identifier: LicenseRef-Blockscout

//! Filecoin-specific charts (chain-wide fees, REV-style:
//! `burn + miner tips`).

pub mod burn_actor_balance;
pub mod fevm_fee_tips;
pub mod filecoin_chain_fees_growth;
pub mod filecoin_new_chain_fees;

/// attoFIL per FIL — the same 10^18 divisor the rest of the crate calls
/// `ETHER`; shared by the SQL statements of this module's charts.
pub(crate) const ETHER: i64 = i64::pow(10, 18);
