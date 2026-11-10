//! The "Push N repo(s)?" question.
use anyhow::{Context, Result};
use std::io::{BufRead, Write};

/// True only for an explicit `y`/`yes`; an empty answer or end of input is no.
pub fn ask(n: usize, input: &mut impl BufRead, out: &mut impl Write) -> Result<bool> {
    write!(out, "Push {n} repo(s)? Runs `git push` in each. [y/N] ")
        .context("writing the question")?;
    out.flush().context("flushing the question")?;
    let mut line = String::new();
    input.read_line(&mut line).context("reading the answer")?;
    Ok(matches!(line.trim().to_lowercase().as_str(), "y" | "yes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(input: &str) -> (Result<bool>, String) {
        let mut out = Vec::new();
        let r = ask(2, &mut input.as_bytes(), &mut out);
        (r, String::from_utf8_lossy(&out).into_owned())
    }

    #[test]
    fn the_question_names_the_count_and_what_will_run() {
        assert_eq!(
            answer("y\n").1,
            "Push 2 repo(s)? Runs `git push` in each. [y/N] "
        );
    }

    #[test]
    fn only_y_and_yes_in_any_case_are_yes() {
        for yes in ["y", "Y", "yes", "YES", " Yes \r"] {
            assert!(answer(&format!("{yes}\n")).0.is_ok_and(|v| v), "{yes:?}");
        }
        for no in ["", "n", "no", "yep", "ok", "1", "yes please"] {
            assert!(answer(&format!("{no}\n")).0.is_ok_and(|v| !v), "{no:?}");
        }
    }

    #[test]
    fn end_of_input_is_no() {
        assert!(answer("").0.is_ok_and(|v| !v));
    }

    #[test]
    fn a_read_error_surfaces() {
        struct Broken;
        impl std::io::Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("boom"))
            }
        }
        let r = ask(1, &mut std::io::BufReader::new(Broken), &mut Vec::new());
        assert!(r.err().is_some_and(|e| format!("{e:#}").contains("boom")));
    }
}
