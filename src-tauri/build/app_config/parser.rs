use std::{collections::HashMap, fmt, path::Path};

pub(crate) struct BuildEnvironment {
    dotenv_path: std::path::PathBuf,
    dotenv_values: HashMap<String, String>,
}

impl BuildEnvironment {
    pub(crate) fn new(dotenv_path: &Path) -> Result<Self, AppConfigParseError> {
        Ok(Self {
            dotenv_path: dotenv_path.to_path_buf(),
            dotenv_values: read_dotenv_values(dotenv_path)?,
        })
    }

    pub(crate) fn emit_cargo_rerun_instructions(&self, env_names: &[&str]) {
        println!("cargo:rerun-if-changed={}", self.dotenv_path.display());
        for name in env_names {
            println!("cargo:rerun-if-env-changed={name}");
        }
    }

    pub(crate) fn parse_non_empty(
        &self,
        name: &'static str,
    ) -> Result<NonEmptyEnvValue, AppConfigParseError> {
        std::env::var(name)
            .ok()
            .or_else(|| self.dotenv_values.get(name).cloned())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(NonEmptyEnvValue)
            .ok_or(AppConfigParseError::Missing { name })
    }
}

fn read_dotenv_values(path: &Path) -> Result<HashMap<String, String>, AppConfigParseError> {
    if !path.is_file() {
        return Ok(HashMap::new());
    }

    dotenvy::from_path_iter(path)
        .map_err(|_| AppConfigParseError::InvalidDotenvFile)?
        .map(|entry| entry.map_err(|_| AppConfigParseError::InvalidDotenvFile))
        .collect()
}

pub(crate) struct NonEmptyEnvValue(String);

impl NonEmptyEnvValue {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

pub(crate) enum AppConfigParseError {
    InvalidDotenvFile,
    Missing { name: &'static str },
}

impl fmt::Display for AppConfigParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDotenvFile => {
                write!(formatter, "invalid build configuration file")
            }
            Self::Missing { name } => {
                write!(
                    formatter,
                    "missing required non-empty build environment variable {name}"
                )
            }
        }
    }
}
