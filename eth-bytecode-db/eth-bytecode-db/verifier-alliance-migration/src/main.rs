// SPDX-License-Identifier: LicenseRef-Blockscout

use sea_orm_migration::prelude::*;

#[tokio::main]
async fn main() {
    cli::run_cli(verifier_alliance_migration::Migrator).await;
}
