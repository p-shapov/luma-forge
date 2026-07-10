use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};

use super::{http::ResponseExt, ProviderError};

pub(super) struct Gql<'a> {
    query: &'a str,
}

impl<'a> Gql<'a> {
    pub(super) fn new(query: &'a str) -> Self {
        Self { query }
    }

    pub(super) fn build(self, variables: Value) -> String {
        json!({ "query": self.query, "variables": variables }).to_string()
    }
}

pub(super) trait GqlResponseExt {
    async fn provider_gql_json<T>(self) -> Result<T, ProviderError>
    where
        T: DeserializeOwned;
}

impl GqlResponseExt for Result<reqwest::Response, reqwest::Error> {
    async fn provider_gql_json<T>(self) -> Result<T, ProviderError>
    where
        T: DeserializeOwned,
    {
        let response = self.provider_json::<GraphqlResponse<T>>().await?;

        if !response.errors.is_empty() {
            return Err(ProviderError::RequestFailed);
        }

        response.data.ok_or(ProviderError::InvalidResponse)
    }
}

#[derive(Deserialize)]
struct GraphqlResponse<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<Value>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Gql;

    #[test]
    fn builds_graphql_request_body() {
        let query = "query Example($id: ID!) { item(id: $id) { id } }\n";
        let variables = json!({ "id": "item-1" });

        let body: serde_json::Value =
            serde_json::from_str(&Gql::new(query).build(variables.clone())).unwrap();

        assert_eq!(body, json!({ "query": query, "variables": variables }));
    }
}
