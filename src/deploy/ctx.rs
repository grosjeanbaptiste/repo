//! What every channel needs: the environment, the project, the world and the two output streams.
use super::target::Env;
use super::tool::Tool;
use super::vars::Vars;
use std::io::Write;
use std::path::Path;

pub struct Ctx<'a> {
    pub env: Env,
    pub root: &'a Path,
    pub vars: &'a Vars,
    pub tool: &'a dyn Tool,
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
}
