use bens_logic::subgraphs_reader::SubgraphReader;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

fn addr(a: &str) -> ethers::types::Address {
    ethers::types::Address::from_slice(hex::decode(a).unwrap().as_slice())
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let url = std::env::var("DATABASE_URL").expect("no database url");
    let pool = Arc::new(
        PgPoolOptions::new()
            .max_connections(40)
            .connect(&url)
            .await?,
    );

    let reader = SubgraphReader::initialize(pool.clone()).await?;
    let domains = reader
        .search_owned_domain_reverse(1, addr("d8da6bf26964af9d7eed9e03e53415d37aa96045"))
        .await?;
    println!("found {} domains for vitalik addr", domains.len());
    println!(
        "{:?}",
        domains
            .iter()
            .map(|d| d.name.as_ref().unwrap().as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let maybe_domain = reader.get_domain(1, "kek.eth").await?;
    println!("found domain = {:?}", maybe_domain);
    Ok(())
}
