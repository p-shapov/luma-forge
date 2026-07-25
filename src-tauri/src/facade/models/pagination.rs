use serde::{Deserialize, Serialize};

use super::RuntimeOperationPageRequest;

#[derive(
    luma_diagnostics::DiagnosticDebug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct PageRequest {
    #[diagnostic(show)]
    pub offset: u64,
    #[diagnostic(show)]
    pub limit: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidPagination;

pub fn validate_page(request: PageRequest) -> Result<(u64, u64), InvalidPagination> {
    (1..=100)
        .contains(&request.limit)
        .then_some((request.offset, request.limit))
        .ok_or(InvalidPagination)
}

pub fn validate_operation_page(
    request: &RuntimeOperationPageRequest,
) -> Result<(u64, u64), InvalidPagination> {
    validate_page(PageRequest {
        offset: request.offset,
        limit: request.limit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_rejects_zero_and_more_than_one_hundred() {
        assert_eq!(
            validate_page(PageRequest {
                offset: 0,
                limit: 0
            }),
            Err(InvalidPagination)
        );
        assert_eq!(
            validate_page(PageRequest {
                offset: 0,
                limit: 101
            }),
            Err(InvalidPagination)
        );
        assert_eq!(
            validate_page(PageRequest {
                offset: 7,
                limit: 100
            }),
            Ok((7, 100))
        );
    }
}
