//! Recipe evaluation engine.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::config::{Action, Condition, Flags, Item, Recipe};
use crate::delivery::{self, DeliveryError, FolderType};
use crate::mail::Message;
use crate::re::Matcher;
use crate::variables::{Env, SubstCtx};

#[cfg(test)]
mod tests;

/// Result of processing all recipes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Message was delivered (to folder, pipe, or forward).
    Delivered(String),
    /// No recipe matched; use default delivery.
    Default,
    /// Processing should continue (copy flag was set).
    Continue,
}

/// Error during recipe evaluation.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("regex error: {0}")]
    Regex(#[from] crate::re::PatternError),
    #[error("delivery error: {0}")]
    Delivery(#[from] DeliveryError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result of engine operations.
pub type EngineResult<T> = Result<T, EngineError>;

/// Mutable state during recipe processing.
#[allow(dead_code)]
pub struct State {
    /// Last condition result (for A flag).
    pub last_cond: bool,
    /// Last action succeeded (for a flag).
    pub last_succ: bool,
    /// Previous condition (for E flag).
    pub prev_cond: bool,
    /// Current score (for weighted conditions).
    pub score: f64,
    /// MATCH variable from `\/` capture.
    pub match_var: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            last_cond: false,
            last_succ: false,
            prev_cond: false,
            score: 0.0,
            match_var: String::new(),
        }
    }
}

/// Recipe evaluation engine.
pub struct Engine<E>
where
    E: Env,
{
    env: E,
    ctx: SubstCtx,
    vars: HashMap<String, String>,
    verbose: bool,
}

impl<E> Engine<E>
where
    E: Env,
{
    pub fn new(env: E, ctx: SubstCtx) -> Self {
        Self {
            env,
            ctx,
            vars: HashMap::new(),
            verbose: false,
        }
    }

    pub fn set_verbose(&mut self, v: bool) {
        self.verbose = v;
    }

    /// Get a variable (local vars override env).
    fn get_var(&self, name: &str) -> Option<String> {
        self.vars.get(name).cloned().or_else(|| self.env.get(name))
    }

    /// Set a local variable.
    pub fn set_var(&mut self, name: &str, value: &str) {
        self.vars.insert(name.to_string(), value.to_string());
    }

    /// Process a list of items (rcfile contents).
    pub fn process(
        &mut self, items: &[Item], msg: &mut Message,
    ) -> EngineResult<Outcome> {
        let mut state = State::default();
        self.process_items(items, msg, &mut state)
    }

    fn process_items(
        &mut self, items: &[Item], msg: &mut Message, state: &mut State,
    ) -> EngineResult<Outcome> {
        for item in items {
            match item {
                Item::Assign { name, value } => {
                    let expanded = self.expand(value);
                    self.set_var(name, &expanded);
                }
                Item::Recipe(recipe) => {
                    let outcome = self.eval_recipe(recipe, msg, state)?;
                    match outcome {
                        Outcome::Delivered(_) => return Ok(outcome),
                        Outcome::Continue | Outcome::Default => {}
                    }
                }
            }
        }
        Ok(Outcome::Default)
    }

    /// Evaluate a single recipe.
    fn eval_recipe(
        &mut self, recipe: &Recipe, msg: &mut Message, state: &mut State,
    ) -> EngineResult<Outcome> {
        if !self.check_chain_flags(&recipe.flags, state) {
            return Ok(Outcome::Default);
        }

        let (matched, score) = self.eval_conditions(recipe, msg, state)?;

        // Update state for next recipe
        if !recipe.flags.chain && !recipe.flags.succ {
            state.last_cond = matched;
        }
        if !recipe.flags.else_ {
            state.prev_cond = matched;
        }

        if !matched {
            return Ok(Outcome::Default);
        }

        let result = self.perform_action(recipe, msg, state)?;

        state.last_succ =
            matches!(result, Outcome::Delivered(_) | Outcome::Continue);
        self.ctx.last_score = score as i64;

        // Copy flag means continue processing even after delivery
        if recipe.flags.copy && matches!(result, Outcome::Delivered(_)) {
            return Ok(Outcome::Continue);
        }

        Ok(result)
    }

    /// Check if chain flags allow this recipe to run.
    fn check_chain_flags(&self, flags: &Flags, state: &State) -> bool {
        // A flag: only if previous condition matched
        if flags.chain && !state.last_cond {
            return false;
        }
        // a flag: only if previous condition matched AND action succeeded
        if flags.succ && !(state.last_cond && state.last_succ) {
            return false;
        }
        // E flag: only if previous condition did NOT match
        if flags.else_ && state.prev_cond {
            return false;
        }
        // e flag: only if previous condition matched but action failed
        if flags.err && (!state.prev_cond || state.last_succ) {
            return false;
        }
        true
    }

    /// Evaluate all conditions, returns (matched, score).
    fn eval_conditions(
        &mut self, recipe: &Recipe, msg: &Message, state: &mut State,
    ) -> EngineResult<(bool, f64)> {
        if recipe.conds.is_empty() {
            return Ok((true, 0.0));
        }

        let mut score = 0.0f64;
        let mut has_score = false;

        for cond in &recipe.conds {
            let (ok, s, scored) =
                self.eval_condition(cond, recipe, msg, state)?;
            if scored {
                score += s;
                has_score = true;
            } else if !ok {
                return Ok((false, score));
            }
        }

        // If we used scoring, match requires score > 0
        if has_score {
            Ok((score > 0.0, score))
        } else {
            Ok((true, 0.0))
        }
    }

    /// Evaluate a single condition. Returns (matched, score_delta, is_scored).
    fn eval_condition(
        &mut self, cond: &Condition, recipe: &Recipe, msg: &Message,
        state: &mut State,
    ) -> EngineResult<(bool, f64, bool)> {
        match cond {
            Condition::Regex { pattern, negate } => {
                self.eval_regex(pattern, *negate, recipe, msg, state)
            }
            Condition::Size { op, bytes } => {
                let size = msg.len() as u64;
                let matched = match op {
                    Ordering::Less => size < *bytes,
                    Ordering::Greater => size > *bytes,
                    Ordering::Equal => size == *bytes,
                };
                if self.verbose {
                    let sym = match op {
                        Ordering::Less => '<',
                        Ordering::Greater => '>',
                        Ordering::Equal => '=',
                    };
                    log::info!(
                        "{} on {}{}",
                        if matched { "Match" } else { "No match" },
                        sym,
                        bytes
                    );
                }
                Ok((matched, 0.0, false))
            }
            Condition::Shell { cmd } => {
                let expanded = self.expand(cmd);
                let text = self.grep_text(msg, &recipe.flags);
                let ok = self.run_shell(&expanded, text.as_bytes())?;
                if self.verbose {
                    log::info!(
                        "{} on ?{}",
                        if ok { "Match" } else { "No match" },
                        cmd
                    );
                }
                Ok((ok, 0.0, false))
            }
            Condition::Variable { name, pattern } => {
                self.eval_variable(name, pattern, recipe, msg, state)
            }
            Condition::Subst { inner, negate } => {
                let (ok, score, scored) =
                    self.eval_condition(inner, recipe, msg, state)?;
                Ok((ok ^ negate, score, scored))
            }
        }
    }

    fn eval_regex(
        &mut self, pattern: &str, negate: bool, recipe: &Recipe, msg: &Message,
        state: &mut State,
    ) -> EngineResult<(bool, f64, bool)> {
        let text = self.grep_text(msg, &recipe.flags);
        let expanded = self.expand(pattern);
        let case_insens = !recipe.flags.case;
        let matcher = Matcher::new(&expanded, case_insens)?;
        let result = matcher.exec(&text);

        if result.matched
            && let Some(cap) = result.capture
        {
            state.match_var = cap.to_string();
            self.set_var("MATCH", cap);
        }

        let matched = result.matched ^ negate;
        if self.verbose {
            log::info!(
                "{} on \"{}\"",
                if matched { "Match" } else { "No match" },
                pattern
            );
        }
        Ok((matched, 0.0, false))
    }

    fn eval_variable(
        &mut self, name: &str, pattern: &str, recipe: &Recipe, msg: &Message,
        state: &mut State,
    ) -> EngineResult<(bool, f64, bool)> {
        let text = self.get_variable_text(name, msg);
        let expanded = self.expand(pattern);
        let case_insens = !recipe.flags.case;
        let matcher = Matcher::new(&expanded, case_insens)?;
        let result = matcher.exec(&text);

        if result.matched
            && let Some(cap) = result.capture
        {
            state.match_var = cap.to_string();
            self.set_var("MATCH", cap);
        }

        if self.verbose {
            log::info!(
                "{} on {} ?? {}",
                if result.matched { "Match" } else { "No match" },
                name,
                pattern
            );
        }
        Ok((result.matched, 0.0, false))
    }

    /// Get text to grep based on H/B flags.
    fn grep_text(&self, msg: &Message, flags: &Flags) -> String {
        let bytes = match (flags.head, flags.body) {
            (true, true) => msg.as_bytes(),
            (true, false) => msg.header(),
            (false, true) => msg.body(),
            (false, false) => msg.header(),
        };
        String::from_utf8_lossy(bytes).into_owned()
    }

    /// Get text for variable condition (VAR ?? pattern).
    fn get_variable_text(&self, name: &str, msg: &Message) -> String {
        match name {
            "H" => String::from_utf8_lossy(msg.header()).into_owned(),
            "B" => String::from_utf8_lossy(msg.body()).into_owned(),
            "HB" | "BH" => String::from_utf8_lossy(msg.as_bytes()).into_owned(),
            _ => self.get_var(name).unwrap_or_default(),
        }
    }

    /// Run a shell command with message as stdin.
    fn run_shell(&mut self, cmd: &str, input: &[u8]) -> EngineResult<bool> {
        let shell = self.get_var("SHELL").unwrap_or_else(|| "/bin/sh".into());
        let mut child = Command::new(&shell)
            .arg("-c")
            .arg(cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(input);
        }

        let status = child.wait()?;
        self.ctx.last_exit = status.code().unwrap_or(-1);
        Ok(status.success())
    }

    /// Perform the recipe's action.
    fn perform_action(
        &mut self, recipe: &Recipe, msg: &mut Message, state: &mut State,
    ) -> EngineResult<Outcome> {
        match &recipe.action {
            Action::Folder(path) => {
                let path_str = path.to_string_lossy();
                let expanded = self.expand(&path_str);
                self.deliver_to_folder(&expanded, recipe, msg)
            }
            Action::Pipe { cmd, capture } => {
                let expanded = self.expand(cmd);
                self.deliver_to_pipe(&expanded, recipe, msg, capture.as_deref())
            }
            Action::Forward(addrs) => {
                let expanded: Vec<_> =
                    addrs.iter().map(|a| self.expand(a)).collect();
                self.forward(recipe, msg, &expanded)
            }
            Action::Nested(items) => self.process_items(items, msg, state),
        }
    }

    /// Deliver to a folder (mbox, maildir, or MH).
    fn deliver_to_folder(
        &mut self, path: &str, recipe: &Recipe, msg: &Message,
    ) -> EngineResult<Outcome> {
        let (folder_type, path) = FolderType::parse(path);
        let msg = self.message_for_delivery(recipe, msg);

        let result = match folder_type {
            FolderType::File => {
                let sender = msg.envelope_sender().unwrap_or("MAILER-DAEMON");
                delivery::mbox(Path::new(path), &msg, sender)?
            }
            FolderType::Maildir => delivery::maildir(Path::new(path), &msg)?,
            FolderType::Mh => delivery::mh(Path::new(path), &msg)?,
            FolderType::Dir => delivery::maildir(Path::new(path), &msg)?,
        };

        self.set_var("LASTFOLDER", &result.path);
        self.ctx.lastfolder = result.path.clone();

        if self.verbose {
            log::info!("Delivered to {}", result.path);
        }

        Ok(Outcome::Delivered(result.path))
    }

    /// Deliver to a pipe command.
    fn deliver_to_pipe(
        &mut self, cmd: &str, recipe: &Recipe, msg: &mut Message,
        capture: Option<&str>,
    ) -> EngineResult<Outcome> {
        let delivery_msg = self.message_for_delivery(recipe, msg);
        let filter = recipe.flags.filter;

        let result = delivery::pipe(cmd, &delivery_msg, filter)?;

        if let Some(ref output) = result.output {
            if filter {
                *msg = Message::parse(output);
            }
            if let Some(var) = capture {
                let text = String::from_utf8_lossy(output);
                self.set_var(var, &text);
                return Ok(Outcome::Continue);
            }
        }

        self.set_var("LASTFOLDER", cmd);
        self.ctx.lastfolder = cmd.to_string();

        if self.verbose {
            log::info!("Piped to \"{}\"", cmd);
        }

        Ok(Outcome::Delivered(cmd.to_string()))
    }

    /// Forward to addresses.
    fn forward(
        &mut self, recipe: &Recipe, msg: &Message, addrs: &[String],
    ) -> EngineResult<Outcome> {
        let sendmail = self
            .get_var("SENDMAIL")
            .unwrap_or_else(|| "/usr/sbin/sendmail".into());
        let msg = self.message_for_delivery(recipe, msg);

        // Skip From_ line for forwarding
        let data = skip_from_line(msg.as_bytes());

        let mut child = Command::new(&sendmail)
            .args(addrs)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(data);
        }

        let status = child.wait()?;
        self.ctx.last_exit = status.code().unwrap_or(-1);

        let dest = addrs.join(" ");
        self.set_var("LASTFOLDER", &dest);
        self.ctx.lastfolder = dest.clone();

        if self.verbose {
            log::info!("Forwarded to {}", dest);
        }

        Ok(Outcome::Delivered(dest))
    }

    /// Create message for delivery based on h/b flags.
    fn message_for_delivery(&self, recipe: &Recipe, msg: &Message) -> Message {
        match (recipe.flags.pass_head, recipe.flags.pass_body) {
            (true, true) => msg.clone(),
            (true, false) => Message::from_parts(msg.header(), &[]),
            (false, true) => Message::from_parts(&[], msg.body()),
            (false, false) => Message::from_parts(&[], &[]),
        }
    }

    /// Expand variables in a string.
    fn expand(&self, s: &str) -> String {
        crate::variables::subst(s, &self.ctx, &EnvWrapper { engine: self })
    }
}

/// Wrapper to make Engine's get_var available via Env trait.
struct EnvWrapper<'a, E>
where
    E: Env,
{
    engine: &'a Engine<E>,
}

impl<E> Env for EnvWrapper<'_, E>
where
    E: Env,
{
    fn get(&self, name: &str) -> Option<String> {
        self.engine.get_var(name)
    }
}

/// Skip mbox From_ line if present.
fn skip_from_line(data: &[u8]) -> &[u8] {
    if data.starts_with(b"From ")
        && let Some(pos) = data.iter().position(|&b| b == b'\n')
    {
        return &data[pos + 1..];
    }
    data
}
