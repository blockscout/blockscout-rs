#![allow(dead_code)]

use bens_logic::subgraphs_reader::{
    blockscout::BlockscoutClient, BatchResolveAddressNamesInput, SubgraphReader,
};
use sqlx::postgres::PgPoolOptions;
use std::{collections::HashMap, sync::Arc, time::Instant};
use tracing_subscriber::fmt::format::FmtSpan;

fn addr(a: &str) -> ethers::types::Address {
    let a = a.trim_start_matches("0x");
    ethers::types::Address::from_slice(hex::decode(a).unwrap().as_slice())
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_span_events(FmtSpan::CLOSE)
        .init();

    let url = std::env::var("DATABASE_URL").expect("no database url");
    let pool = Arc::new(
        PgPoolOptions::new()
            .max_connections(40)
            .connect(&url)
            .await?,
    );
    let eth_client = BlockscoutClient::new("https://eth.blockscout.com".parse().unwrap(), 5, 30);
    let rootstock_client =
        BlockscoutClient::new("https://rootstock.blockscout.com".parse().unwrap(), 5, 30);
    let clients: HashMap<i64, BlockscoutClient> =
        HashMap::from_iter([(1, eth_client), (30, rootstock_client)]);
    let reader = SubgraphReader::initialize(pool.clone(), clients).await?;

    let addresses = vec![
        "0x0292f204513eeafe8c032ffc4cb4c7e10eca908c",
        "0x04e2322214af4f3c39a8481c4b105ac1c1c3d190",
        "0x0561aadd60629ee01556c1b06ed334d300468dc1",
        "0x05a107ae64cf029086ee996d2599f09cc4f2274e",
        "0x0bbf4e920352811629dd634ffdfe4383d366bef8",
        "0x5FA53C4f055BD1ba0380b45fC85e39D79B2975BA",
        "0xA9cC5C0A9C05Fe991747DAAd8A3Dcf2758570359",
        "0x282bDAfD8d360d1d8f99D76620206D3DBB19372D",
        "0xfdfd5754413fADe7aF987935C08122060526A9Fa",
        "0xe34139463bA50bD61336E0c446Bd8C0867c6fE65",
        "0xBc7fE8A6E12eCB83aE51619aBfbCC45a43ac88ab",
        "0x0CD04F925C5885C876D74170bb4940BFbC5db47C",
        "0xFF29146F27fc65e82E9c7072c86c40fb3835576D",
        "0x2c38a56765d39E7FB18A040E974f942F4C67d80f",
        "0x85146EE726f7fcD06b8A9bc969f41752Fe784b20",
        "0x27899ffaCe558bdE9F284Ba5C8c91ec79EE60FD6",
        "0x9906A2E0cf76A2DC8B8D11f5C66b204181B6416C",
        "0xa612212E50edbB810caf4B176582100Dff32095E",
        "0x2E91bAc65Dd522A211aB4eF448e365B02a7859f9",
        "0x9e8073c31266E9F3fd81b278c8EFaDb156318481",
        "0x466324DC848bD9D0074F118E6CbA3C55E8C54237",
        "0xdA393DE4F410a349ABb13C52DEE36c8AaBaC7851",
        "0x63bFd6525ce5Dcef8ce6ACa483488bD111e3ff55",
        "0x349B7daccEeBB40658C6D6e8cc99aa36b047F992",
        "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
        "0xdAC17F958D2ee523a2206206994597C13D831ec7",
        "0x91365681900536A2211658e90Bd0973C202f5622",
        "0xFe35B83BC45B1Ae677911AeCA84941A7Eb16BaA2",
        "0xf003281EdfaFC87B703A768dA8804ac865BA7778",
        "0x8C1a16263e8784d26E6654c88b39e667E38A8F01",
        "0x1b9e782750f11ad03f85ffb98CBFCc2276f33961",
        "0xC1427362699c1c7438cFA8A65323dDbce1c61031",
        "0xBBe34c189a6F23ba9C1114c74265d42c0648FBcD",
        "0x5d22045DAcEAB03B158031eCB7D9d06Fad24609b",
        "0x603CfF6c9a0bD734589C92Ee73c93e77D3764DD8",
        "0x49BCbc58848AC642B48f3D824D8b728a1045D08b",
        "0x7f6b0b7a17E5f8EF43e824f93cD006E98B632140",
        "0x0ef303a549722d0DDe364c430512E10C907cD510",
        "0x0C657d6C37954535391d9AA5B79822ABBA91DF78",
        "0x52aBb054a58677137D1ca0EB7a7cf9DdD3D683eB",
        "0xA461382E69d081A79d510e31fC7f35cbD7a7695E",
        "0xc09B6E4C1077a725E3f39C0C3B9f466a51EDE04b",
        "0x6700132b20ea19bA30f4ca78263F3BA245039539",
        "0x2f08c107D6B6454B38459464922AB7E97FFBe9D9",
        "0x55DCad916750C19C4Ec69D65Ff0317767B36cE90",
        "0x67E263e8497aD043E4633f8d04e65F29dCfD730b",
        "0x441761326490cACF7aF299725B6292597EE822c2",
        "0xC874b064f465bdD6411D45734b56fac750Cda29A",
        "0x0902e87ff457d7d85D674E7fe0c66442791b1654",
        "0xf6eCa9A0F4081Fec383bff83045B417149F3a388",
        "0xDDcEc5032109d85f43899ddbAcFbb576C245E42F",
        "0xf7697EE54F726fE2691f3D3bd1AfABE852FC4E01",
        "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D",
        "0x27596318801604CA4b53aaDbf4076CaB6805E52B",
        "0x42F0e1dbD3c6F2B21F0dF02B68275E706caD994d",
        "0x253553366Da8546fC250F225fe3d25d0C782303b",
        "0xb09630e39bf213d56FC272F0e39102E7106ccAaF",
        "0x0000F2168ba1343Ca8D7Cc54114264340A4A0000",
        "0x0a5b058560e2Db597f57FedB910f3C2F50F4438C",
        "0x72B886d09C117654aB7dA13A14d603001dE0B777",
        "0x70B2A3e586c7f3eAE3E781D3e562922C6E1A96E1",
        "0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD",
        "0x2E39F19493073E9d1422a3A48494DE9Fb9A07Edb",
        "0x1f9090aaE28b8a3dCeaDf281B0F12828e676c326",
        "0xae0Ee0A63A2cE6BaeEFFE56e7714FB4EFE48D419",
        "0x524F2Bd07C57850f1E18Ac0fB11a58cd4f3acba7",
        "0x2B47f29bE67a67995cf33d80DE7EFE0160720b72",
        "0xD8C027cB6dBBecB41F3184F62E6281DAb531fCa8",
        "0x20718EfbC25Dba60FD51c2c81362b83f7C411A6D",
        "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
        "0x58edF78281334335EfFa23101bBe3371b6a36A51",
        "0x9C19B0497997Fe9E75862688a295168070456951",
        "0x55FE002aefF02F77364de339a1292923A15844B8",
        "0x24145F16c7119f141C87F26DB2A07387941D3709",
        "0xEf42750EB260C851dC7F30716B3E43Ba42299cE8",
        "0x48a20c2be7E103770f48e68ED4838f9d960f073d",
        "0x7eB1821aC65c5387E82eF2e26eAE7c49E7Ba0F19",
        "0xAa41A27843B3F440953C1F4C7D9cb42078B263ba",
        "0xB62E45c3Df611dcE236A6Ddc7A493d79F9DFadEf",
        "0xA2Fde9De6e6eBFE068d3eC0B515669817a9c3808",
        "0xa968cf59aB2BaE618f6eE0a80EcBd5b242ebE991",
        "0x0E776D04240575dFfcF10646Cd2d098D7969aEd6",
        "0x7F6aAe679dC0bD7d6ecF62224A5a3423877d6Be7",
        "0x86f14148A3699b665B2d6AC9107561a6610167D4",
        "0x9B1C20ee98C3b6070a9646B677BE5f4c0b388fec",
        "0xB01cb49fe0D6D6E47EDf3A072d15dfe73155331C",
        "0x0D0707963952f2fBA59dD06f2b425ace40b492Fe",
        "0x4bfa051C86d13F4d968E8A0E7af62B47CCc960e2",
        "0x88E5aaA13aca214cD3fd3f649E86bB3782E3e6F6",
        "0x26bb61eF231A110Ef2B0aF004650fb092fb90C83",
        "0xad3b67BCA8935Cb510C8D18bD45F0b94F54A968f",
        "0xffe15FF598e719d29DFe5E1d60BE1A5521A779Ae",
        "0xb000b05c543ddcfdb938b304d40758357210ba17",
        "0x91b99c9b75af469a71ee1ab528e8da994a5d7030",
    ]
    .into_iter()
    .map(addr)
    .collect::<Vec<_>>();
    let size = addresses.len();
    let now = Instant::now();
    let result = reader
        .batch_resolve_address_names(BatchResolveAddressNamesInput {
            network_id: 1,
            addresses,
        })
        .await
        .expect("failed to quick resolve");
    // job size is 94. elapsed 1.1955539s. resolved as 13 domains
    println!(
        "job size is {}. elapsed {:?}s. resolved as {} domains",
        size,
        now.elapsed().as_secs_f32(),
        result.len()
    );
    Ok(())
}
