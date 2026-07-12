use graphql_client::GraphQLQuery;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "../graphql/runpod.graphql",
    query_path = "src/infra/clients/runpod/queries/myself.graphql"
)]
pub struct Myself;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "../graphql/runpod.graphql",
    query_path = "src/infra/clients/runpod/queries/placement.graphql"
)]
pub struct Placement;
