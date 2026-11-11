//! Where a deploy goes: the environment and the platform (port of `project-deploy.sh` routing).
use anyhow::{Result, bail};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Env {
    Prod,
    Test,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Target {
    Web,
    Ios,
    Android,
    Mas,
    Homebrew,
    Windows,
    Linux,
}

/// One target, or every concrete target (`all`).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Choice {
    One(Target),
    All,
}

/// The targets `all` fans out to, in order.
pub const CONCRETE: [Target; 7] = [
    Target::Web,
    Target::Ios,
    Target::Android,
    Target::Mas,
    Target::Homebrew,
    Target::Windows,
    Target::Linux,
];

impl Env {
    pub fn parse(s: &str) -> Result<Env> {
        match s {
            "prod" => Ok(Env::Prod),
            "test" => Ok(Env::Test),
            _ => bail!("invalid env '{s}' — expected: prod | test"),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Env::Prod => "prod",
            Env::Test => "test",
        }
    }
}

impl Target {
    pub fn name(self) -> &'static str {
        match self {
            Target::Web => "web",
            Target::Ios => "ios",
            Target::Android => "android",
            Target::Mas => "mas",
            Target::Homebrew => "homebrew",
            Target::Windows => "windows",
            Target::Linux => "linux",
        }
    }
}

impl Choice {
    pub fn parse(s: &str) -> Result<Choice> {
        if s == "all" {
            return Ok(Choice::All);
        }
        match CONCRETE.into_iter().find(|t| t.name() == s) {
            Some(t) => Ok(Choice::One(t)),
            None => {
                let names: Vec<&str> = CONCRETE.iter().map(|t| t.name()).collect();
                bail!("invalid target '{s}' — expected: {} | all", names.join(" "))
            }
        }
    }

    pub fn targets(self) -> Vec<Target> {
        match self {
            Choice::All => CONCRETE.to_vec(),
            Choice::One(t) => vec![t],
        }
    }
}

#[cfg(test)]
mod tests {
    include!("target_tests.rs");
}
