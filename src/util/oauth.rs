//! OAuth utilities
//!
//! Bridges `reqwest::Client` with `oauth2::AsyncHttpClient` so that
//! oauth2 can be used without pulling in its default reqwest feature.

use std::future::Future;
use std::pin::Pin;

/// Bridges reqwest::Client with oauth2::AsyncHttpClient
pub struct ReqwestClient(pub reqwest::Client);

impl<'c> oauth2::AsyncHttpClient<'c> for ReqwestClient {
    type Error = oauth2::HttpClientError<reqwest::Error>;

    type Future =
        Pin<Box<dyn Future<Output = Result<oauth2::HttpResponse, Self::Error>> + Send + Sync + 'c>>;

    fn call(&'c self, request: oauth2::HttpRequest) -> Self::Future {
        Box::pin(async move {
            let response = self
                .0
                .execute(request.try_into().map_err(Box::new)?)
                .await
                .map_err(Box::new)?;

            let mut builder = http::Response::builder()
                .status(response.status())
                .version(response.version());

            for (name, value) in response.headers().iter() {
                builder = builder.header(name, value);
            }

            builder
                .body(response.bytes().await.map_err(Box::new)?.to_vec())
                .map_err(oauth2::HttpClientError::Http)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reqwest_client_creation() {
        let client = reqwest::Client::new();
        let _reqwest_client = ReqwestClient(client);
    }

    #[test]
    fn test_reqwest_client_with_timeout() {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();
        let _reqwest_client = ReqwestClient(client);
    }

    #[test]
    fn test_reqwest_client_with_user_agent() {
        let client = reqwest::Client::builder()
            .user_agent("test-agent/1.0")
            .build()
            .unwrap();
        let _reqwest_client = ReqwestClient(client);
    }

    #[test]
    fn test_reqwest_client_default() {
        let client = reqwest::Client::default();
        let _reqwest_client = ReqwestClient(client);
    }

    #[test]
    fn test_reqwest_client_clone() {
        let client = reqwest::Client::new();
        let reqwest_client = ReqwestClient(client);
        // Verify the struct can be cloned (reqwest::Client is Clone)
        let _cloned = reqwest_client.0.clone();
    }

    #[test]
    fn test_reqwest_client_send_sync() {
        // Verify ReqwestClient is Send and Sync (required for async use)
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ReqwestClient>();
        assert_sync::<ReqwestClient>();
    }
}
