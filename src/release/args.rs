//! Arguments of `repo release`: our flags first, everything else forwarded to the release script.
use anyhow::{Result, bail};

#[derive(Debug, PartialEq, Eq, Default)]
pub struct Opts {
    pub require_audit: bool,
    pub path: Option<String>,
    pub script: Option<String>,
    pub forward: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Parsed {
    Help,
    Run(Opts),
}

/// Reads our flags up to the first argument that is not one of them (or `--`);
/// the rest goes to the release script verbatim, unknown `--flags` included.
pub fn parse(args: &[String]) -> Result<Parsed> {
    let mut o = Opts::default();
    let mut rest = args.iter();
    let value = |flag: &str, rest: &mut std::slice::Iter<String>| -> Result<String> {
        match rest.next() {
            Some(v) => Ok(v.clone()),
            None => bail!("{flag} needs a value"),
        }
    };
    while let Some(a) = rest.clone().next() {
        match a.as_str() {
            "--require-audit" => o.require_audit = true,
            "--path" => {
                rest.next();
                o.path = Some(value("--path", &mut rest)?);
                continue;
            }
            "--script" => {
                rest.next();
                o.script = Some(value("--script", &mut rest)?);
                continue;
            }
            "-h" | "--help" | "help" => return Ok(Parsed::Help),
            "--" => {
                rest.next();
                break;
            }
            _ if a.starts_with("--path=") => o.path = a.strip_prefix("--path=").map(String::from),
            _ if a.starts_with("--script=") => {
                o.script = a.strip_prefix("--script=").map(String::from)
            }
            _ => break,
        }
        rest.next();
    }
    o.forward = rest.cloned().collect();
    Ok(Parsed::Run(o))
}

#[cfg(test)]
mod tests {
    include!("args_tests.rs");
}
