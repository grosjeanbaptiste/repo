//! Name selection: actname proposes, the caller's `taken` check filters.
use actname::{InMemoryLexicon, NamingCriteria, NamingService, Strategy};
use anyhow::{Context, Result};

pub struct Query {
    pub prefixes: Vec<String>,
    pub domain: String,
    pub max_length: usize,
    pub limit: usize,
}

pub fn candidates(q: &Query) -> Result<Vec<String>> {
    let criteria = NamingCriteria {
        prefixes: q.prefixes.clone(),
        context: q.domain.clone(),
        max_length: q.max_length,
        limit: q.limit,
        strategy: Strategy::Hybrid,
        exclusions: vec![],
    };
    let found = NamingService::new(&InMemoryLexicon)
        .generate(&criteria)
        .context("actname refused the request")?;
    Ok(found.into_iter().map(|c| c.name).collect())
}

/// First candidate for which `taken` is false; a failing check aborts.
pub fn first_free(names: &[String], mut taken: impl FnMut(&str) -> Result<bool>) -> Result<String> {
    for name in names {
        if !taken(name)? {
            return Ok(name.clone());
        }
    }
    anyhow::bail!("no free name among {} actname candidates", names.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Vec<String> {
        ["a", "b", "c"].map(String::from).to_vec()
    }

    #[test]
    fn skips_taken_names() {
        assert_eq!(
            first_free(&names(), |n| Ok(n == "a")).unwrap_or_default(),
            "b"
        );
    }

    #[test]
    fn fails_when_all_taken() {
        let err = first_free(&names(), |_| Ok(true))
            .err()
            .map(|e| e.to_string());
        assert!(err.is_some_and(|e| e.contains("no free name")));
    }

    #[test]
    fn check_failure_aborts() {
        let err = first_free(&names(), |_| anyhow::bail!("boom"))
            .err()
            .map(|e| e.to_string());
        assert_eq!(err.as_deref(), Some("boom"));
    }

    #[test]
    fn git_tooling_domain_has_candidates() {
        let q = Query {
            prefixes: vec!["act".into()],
            domain: "git repositories tooling".into(),
            max_length: 10,
            limit: 20,
        };
        assert!(!candidates(&q).unwrap_or_default().is_empty());
    }

    #[test]
    fn missing_prefix_is_an_error() {
        let q = Query {
            prefixes: vec![],
            domain: "git".into(),
            max_length: 10,
            limit: 5,
        };
        assert!(candidates(&q).is_err());
    }
}
