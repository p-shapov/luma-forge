use syn::{parse::Parse, Attribute, Result};

#[derive(Clone, Copy)]
pub enum ValuePolicy {
    Omit,
    Show,
    Redact,
}

pub struct FunctionArgs {
    pub root: bool,
    pub output: ValuePolicy,
    pub error: ValuePolicy,
}

impl Default for FunctionArgs {
    fn default() -> Self {
        Self {
            root: false,
            output: ValuePolicy::Omit,
            error: ValuePolicy::Omit,
        }
    }
}

impl Parse for FunctionArgs {
    fn parse(input: syn::parse::ParseStream<'_>) -> Result<Self> {
        let mut args = Self::default();
        let metas = input.parse_terminated(syn::Meta::parse, syn::Token![,])?;

        for meta in metas {
            if !matches!(&meta, syn::Meta::Path(_)) {
                return Err(syn::Error::new_spanned(
                    meta,
                    "expected a diagnostic policy flag",
                ));
            }
            let path = meta.path();
            if path.is_ident("root") {
                if args.root {
                    return Err(syn::Error::new_spanned(meta, "duplicate `root` policy"));
                }
                args.root = true;
            } else if path.is_ident("show_output") {
                set_policy(&mut args.output, ValuePolicy::Show, &meta, "output")?;
            } else if path.is_ident("redact_output") {
                set_policy(&mut args.output, ValuePolicy::Redact, &meta, "output")?;
            } else if path.is_ident("show_error") {
                set_policy(&mut args.error, ValuePolicy::Show, &meta, "error")?;
            } else if path.is_ident("redact_error") {
                set_policy(&mut args.error, ValuePolicy::Redact, &meta, "error")?;
            } else {
                return Err(syn::Error::new_spanned(
                    meta,
                    "expected `root`, `show_output`, `redact_output`, `show_error`, or `redact_error`",
                ));
            }
        }

        Ok(args)
    }
}

fn set_policy(
    policy: &mut ValuePolicy,
    next: ValuePolicy,
    meta: &syn::Meta,
    name: &str,
) -> Result<()> {
    if !matches!(policy, ValuePolicy::Omit) {
        return Err(syn::Error::new_spanned(
            meta,
            format!("duplicate or conflicting diagnostic {name} policy"),
        ));
    }
    *policy = next;
    Ok(())
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
