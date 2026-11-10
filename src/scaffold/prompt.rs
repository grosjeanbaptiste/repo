//! Ask for the project type on stdin when `repo new` got none
//! (the line-prompt path of `dotfiles project setup`; no fzf).
use anyhow::{Context, Result, bail};
use std::io::{BufRead, Write};

pub fn ask(types: &[String], input: &mut impl BufRead, out: &mut impl Write) -> Result<String> {
    write!(out, "Type ({}): ", types.join("/")).context("writing the type prompt")?;
    out.flush().context("flushing the type prompt")?;
    let mut line = String::new();
    input
        .read_line(&mut line)
        .context("reading the project type")?;
    let kind = line.trim();
    if kind.is_empty() {
        bail!(
            "new needs a project type. Available types: {}",
            types.join(", ")
        );
    }
    Ok(kind.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn types() -> Vec<String> {
        ["app/rust", "doc/tex", "workspace"]
            .map(String::from)
            .to_vec()
    }

    fn ask_with(input: &str) -> (Result<String>, String) {
        let mut out = Vec::new();
        let r = ask(&types(), &mut input.as_bytes(), &mut out);
        (r, String::from_utf8_lossy(&out).into_owned())
    }

    #[test]
    fn the_prompt_lists_every_type() {
        let (_, out) = ask_with("app/rust\n");
        assert_eq!(out, "Type (app/rust/doc/tex/workspace): ");
    }

    #[test]
    fn the_answer_is_trimmed() {
        assert_eq!(
            ask_with("  doc/tex \r\n").0.ok().as_deref(),
            Some("doc/tex")
        );
    }

    #[test]
    fn only_the_first_line_is_read() {
        assert_eq!(
            ask_with("workspace\napp/rust\n").0.ok().as_deref(),
            Some("workspace")
        );
    }

    #[test]
    fn an_unknown_answer_is_returned_for_the_caller_to_refuse() {
        assert_eq!(ask_with("cobol\n").0.ok().as_deref(), Some("cobol"));
    }

    #[test]
    fn empty_answer_eof_and_blank_all_fail_naming_the_types() {
        for input in ["\n", "", "   \n"] {
            let e = ask_with(input)
                .0
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default();
            assert!(
                e.contains("needs a project type") && e.contains("app/rust, doc/tex, workspace"),
                "{input:?}: {e}"
            );
        }
    }

    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("boom"))
        }
    }

    #[test]
    fn a_read_error_surfaces() {
        let mut input = std::io::BufReader::new(Broken);
        let r = ask(&types(), &mut input, &mut Vec::new());
        assert!(r.err().is_some_and(|e| format!("{e:#}").contains("boom")));
    }

    struct FullDisk;
    impl Write for FullDisk {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("disk full"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_write_error_surfaces() {
        let r = ask(&types(), &mut "x\n".as_bytes(), &mut FullDisk);
        assert!(
            r.err()
                .is_some_and(|e| format!("{e:#}").contains("disk full"))
        );
    }
}
