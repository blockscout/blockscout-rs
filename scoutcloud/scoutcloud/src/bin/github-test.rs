use blockscout_service_launcher::launcher::ConfigSettings;
use scoutcloud::{logic::github::Workflow, server::Settings};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");
    let client = scoutcloud::logic::github::GithubClient::new(
        settings.github.token,
        settings.github.owner,
        settings.github.repo,
        "main".to_string(),
        None,
    )?;

    let r = scoutcloud::logic::github::DeployWorkflow::get_latest_run(&client, None)
        .await?
        .unwrap();
    println!("{}: {} - {}", r.id, r.name, r.status);
    let r = scoutcloud::logic::github::DeployWorkflow {
        client: "sevenzing-test-2".to_string(),
    }
    .run_and_get_latest_with_mutex(&client, 5)
    .await?
    .unwrap();
    println!("{}: {} - {}", r.id, r.name, r.status);
    Ok(())
}
