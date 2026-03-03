use thiserror::Error;

#[derive(Debug, Error)]
pub enum CcsError {
    #[error("config error: {0}")]
    Config(String),

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("environment variable ${{{var}}} is not set")]
    EnvVar { var: String },

    #[error("profile '{name}' not found. Available: {available}")]
    ProfileNotFound { name: String, available: String },

    #[error(
        "Claude Code not found in PATH.\n\
         Install it with: npm install -g @anthropic-ai/claude-code"
    )]
    BinaryNotFound,

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("{0}")]
    Other(String),
}
