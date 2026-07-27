// SPDX-License-Identifier: LicenseRef-Blockscout

use reqwest::Url;
use std::{num::NonZeroU32, time::Duration};

pub struct SourcifyApiClient {
    lib_client: sourcify::Client,
}

impl SourcifyApiClient {
    /// Initialize new sourcify client.
    pub fn new(
        host: Url,
        request_timeout: u64,
        verification_attempts: NonZeroU32,
        poll_interval: Duration,
        max_poll_attempts: NonZeroU32,
    ) -> Result<Self, reqwest::Error> {
        let lib_client = sourcify::ClientBuilder::default()
            .try_base_url(host.as_str())
            .expect("valid sourcify base url")
            .max_retries(verification_attempts.get())
            .request_timeout(Duration::from_secs(request_timeout))
            .poll_interval(poll_interval)
            .max_poll_attempts(max_poll_attempts.get())
            .build();

        Ok(Self { lib_client })
    }

    pub fn lib_client(&self) -> &sourcify::Client {
        &self.lib_client
    }
}
