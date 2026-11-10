//! Where a referenced variable is fulfilled on this machine.
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    InEnv,
    /// Enrolled in the dotfiles secret store under this name, but not exported.
    InDotfiles(String),
    Missing,
}

/// The dotfiles secret that backs a well-known variable.
pub fn known_secret(var: &str) -> Option<&'static str> {
    Some(match var {
        "GH_TOKEN" | "GITHUB_TOKEN" | "GITHUB_AUTH_TOKEN" | "GH_API_TOKEN" => "gh-token",
        "HF_TOKEN" | "HUGGINGFACE_TOKEN" | "HUGGING_FACE_TOKEN" | "HUGGINGFACE_HUB_TOKEN" => {
            "hf-token"
        }
        "GITLAB_TOKEN" | "GL_TOKEN" | "GITLAB_API_TOKEN" | "GLAB_TOKEN" | "CI_JOB_TOKEN" => {
            "glab-config"
        }
        "NPM_TOKEN" | "NODE_AUTH_TOKEN" => "npmrc",
        "ASC_KEY_ID" | "ASC_ISSUER_ID" | "ASC_PRIVATE_KEY_PATH" | "ASC_KEY_PATH" => "asc-api-key",
        _ => return None,
    })
}

/// `in_env` and `enrolled` are the two lookups: the process environment and the secret store.
pub fn status(var: &str, in_env: &dyn Fn(&str) -> bool, enrolled: &dyn Fn(&str) -> bool) -> Status {
    if in_env(var) {
        return Status::InEnv;
    }
    match known_secret(var) {
        Some(secret) if enrolled(secret) => Status::InDotfiles(secret.to_string()),
        _ => Status::Missing,
    }
}

/// Whether `<state root>/secrets/<name>.age` can be read.
pub fn enrolled(state_root: &Path, secret: &str) -> bool {
    std::fs::File::open(state_root.join("secrets").join(format!("{secret}.age"))).is_ok()
}

#[cfg(test)]
mod tests {
    include!("resolve_tests.rs");
}
