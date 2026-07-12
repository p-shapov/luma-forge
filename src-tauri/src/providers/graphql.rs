use serde::de::DeserializeOwned;

use super::{http::ResponseExt, NetworkError};

pub(super) trait GraphqlResponseExt {
    async fn into_graphql_data<T>(self) -> Result<T, NetworkError>
    where
        T: DeserializeOwned;
}

impl GraphqlResponseExt for Result<reqwest::Response, reqwest::Error> {
    async fn into_graphql_data<T>(self) -> Result<T, NetworkError>
    where
        T: DeserializeOwned,
    {
        let response = self.into_json::<graphql_client::Response<T>>().await?;

        if response.errors.is_some_and(|errors| !errors.is_empty()) {
            return Err(NetworkError::RequestFailed);
        }

        response.data.ok_or(NetworkError::InvalidResponse)
    }
}
