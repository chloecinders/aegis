use std::time::Duration;

use reqwest::Client;

#[derive(Clone, Debug)]
pub struct Http {
    client: Client,
}

#[derive(Debug, thiserror::Error)]
pub enum Failure {
    #[error("transport: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("{url} is {size} bytes, over the {limit} byte ceiling")]
    TooLarge {
        url: String,
        size: usize,
        limit: usize,
    },
}

impl Default for Http {
    fn default() -> Self {
        Self::new()
    }
}

// Discord drops idle keep-alive connections well before reqwest retires them after its
// default ninety seconds, and a request written onto one that has already gone away
// fails as a bare transport error with nothing to retry it. Log entries arrive in bursts
// separated by long quiet stretches, which is exactly the shape that keeps landing on a
// dead connection, so retire pooled connections early and cap a request that stalls.
pub fn discord() -> Client {
    Client::builder()
        .use_rustls_tls()
        .pool_idle_timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap_or_default()
}

impl Http {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent(concat!("Aegis/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub async fn bytes(&self, url: &str, limit: usize) -> Result<Vec<u8>, Failure> {
        let response = self.client.get(url).send().await?.error_for_status()?;

        if let Some(declared) = response.content_length()
            && declared as usize > limit
        {
            return Err(Failure::TooLarge {
                url: url.to_string(),
                size: declared as usize,
                limit,
            });
        }

        let body = response.bytes().await?;

        if body.len() > limit {
            return Err(Failure::TooLarge {
                url: url.to_string(),
                size: body.len(),
                limit,
            });
        }

        Ok(body.to_vec())
    }
}
