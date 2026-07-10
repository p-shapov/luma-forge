use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};

use super::{http::ResponseExt, NetworkError};

pub(super) struct GraphqlRequest<'a> {
    query: &'a str,
}

impl<'a> GraphqlRequest<'a> {
    pub(super) fn new(query: &'a str) -> Self {
        Self { query }
    }

    pub(super) fn build(self, variables: Value) -> String {
        json!({ "query": self.query, "variables": variables }).to_string()
    }
}

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
        let response = self.into_json::<GraphqlResponse<T>>().await?;

        if !response.errors.is_empty() {
            return Err(NetworkError::RequestFailed);
        }

        response.data.ok_or(NetworkError::InvalidResponse)
    }
}

#[derive(Deserialize)]
struct GraphqlResponse<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<Value>,
}
