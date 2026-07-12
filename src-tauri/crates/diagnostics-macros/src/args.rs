use syn::{Attribute, Result};

#[derive(Clone, Copy)]
pub enum ValuePolicy {
    Omit,
    Show,
    Redact,
}

pub fn value_policy(attributes: &[Attribute]) -> Result<ValuePolicy> {
    let mut policy = ValuePolicy::Omit;

    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("diagnostic"))
    {
        attribute.parse_nested_meta(|meta| {
            let next = if meta.path.is_ident("show") {
                ValuePolicy::Show
            } else if meta.path.is_ident("redact") {
                ValuePolicy::Redact
            } else {
                return Err(meta.error("expected `show` or `redact`"));
            };

            if !matches!(policy, ValuePolicy::Omit) {
                return Err(meta.error("duplicate or conflicting diagnostic field policy"));
            }
            policy = next;
            Ok(())
        })?;
    }

    Ok(policy)
}
