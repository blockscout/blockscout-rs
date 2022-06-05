use crate::http_server::handlers::verification::sourcify;

pub(super) async fn verification_request(
    params: &sourcify::types::ApiRequest,
    sourcify_api_url: &str,
) -> Result<sourcify::types::ApiVerificationResponse, reqwest::Error> {
    let resp = reqwest::Client::new()
        .post(sourcify_api_url)
        .json(&params)
        .send()
        .await?;

    resp.json().await
}

pub(super) async fn verification_files(
    params: &sourcify::types::ApiRequest,
    sourcify_api_url: &str,
) -> Result<sourcify::types::ApiFilesResponse, reqwest::Error> {
    let url = format!(
        "{}/files/any/{}/{}",
        sourcify_api_url, params.chain, params.address
    );
    let resp = reqwest::Client::new().get(url).send().await?;

    resp.json().await
}
