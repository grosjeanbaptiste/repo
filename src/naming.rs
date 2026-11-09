//! Name selection: actname proposes, the caller's `taken` check filters.
use actit::name::{NameStatus, check_anywhere};
use actit::ports::RepoNames;
use actname::{InMemoryLexicon, NamingCriteria, NamingService, Strategy};
use anyhow::{Context, Result};
use std::path::Path;

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

/// A name is taken when its directory exists or actit finds it on GitHub, under any owner.
pub fn taken(root: &Path, name: &str, names: &impl RepoNames) -> Result<bool> {
    if root.join(name).exists() {
        return Ok(true);
    }
    Ok(check_anywhere(name, names)?.status == NameStatus::Taken)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actit::contains::Listing;
    use actit::error::Error;

    struct Registry(Result<Vec<String>, Error>);

    impl RepoNames for Registry {
        fn owner(&self) -> Result<String, Error> {
            Ok("me".into())
        }
        fn exists(&self, _: &str, _: &str) -> Result<bool, Error> {
            Ok(false)
        }
        fn search(&self, _: &str) -> Result<Vec<String>, Error> {
            self.0.clone()
        }
        fn contains(&self, _: &str) -> Result<Listing, Error> {
            Ok(Listing {
                total: 0,
                names: vec![],
            })
        }
    }

    fn taken_by(found: &[&str]) -> Registry {
        Registry(Ok(found.iter().map(|s| s.to_string()).collect()))
    }

    #[test]
    fn existing_directory_is_taken_without_asking_github() {
        let dir = std::env::temp_dir();
        let boom = Registry(Err(Error::Github("must not be called".into())));
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let parent = dir.parent().unwrap_or(&dir);
        assert!(taken(parent, name, &boom).unwrap_or(false));
    }

    #[test]
    fn name_found_under_another_owner_is_taken() {
        let root = Path::new("/no/such/root");
        assert!(taken(root, "pr", &taken_by(&["bob/pr"])).unwrap_or(false));
    }

    #[test]
    fn name_nobody_has_is_free() {
        let root = Path::new("/no/such/root");
        assert!(!taken(root, "zzqq", &taken_by(&[])).unwrap_or(true));
    }

    #[test]
    fn github_failure_is_an_error() {
        let root = Path::new("/no/such/root");
        let boom = Registry(Err(Error::Github("rate limit".into())));
        assert!(taken(root, "pr", &boom).is_err());
    }

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
