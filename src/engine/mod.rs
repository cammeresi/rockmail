//! Recipe evaluation engine.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use nix::sys::stat::{self, Mode};
use nix::unistd::dup2;

use crate::config::{Action, Condition, Flags, Item, Recipe, Weight};
use crate::delivery::{self, DeliveryError, DeliveryOpts, FolderType, Namer};
use crate::locking::FileLock;
use crate::mail::Message;
use crate::re::Matcher;
use crate::variables::{
    DEF_LOCKEXT, DEF_LOCKSLEEP, DEF_LOCKTIMEOUT, DEF_SENDMAIL,
    DEF_SENDMAILFLAGS, DEF_SHELL, DEF_SHELLFLAGS, DEV_NULL, Environment,
    SubstCtx, VAR_EXITCODE, VAR_HOST, VAR_LOCKEXT, VAR_LOCKFILE, VAR_LOCKSLEEP,
    VAR_LOCKTIMEOUT, VAR_LOG, VAR_LOGFILE, VAR_MAILDIR, VAR_SENDMAIL,
    VAR_SENDMAILFLAGS, VAR_SHELL, VAR_SHELLFLAGS, VAR_SHIFT, VAR_TRAP,
    VAR_UMASK, VAR_VERBOSE, value_as_int,
};

#[cfg(test)]
mod tests;

const MAX_INCLUDE_DEPTH: usize = 32;
const MAX32: f64 = i32::MAX as f64;
const MIN32: f64 = i32::MIN as f64;

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
    /// Regex compilation or matching error.
    #[error("regex error: {0}")]
    Regex(#[from] crate::re::PatternError),
    /// Delivery failure (folder, pipe, or forward).
    #[error("delivery error: {0}")]
    Delivery(#[from] DeliveryError),
    /// I/O error during processing.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Failed to acquire a lockfile.
    #[error("failed to acquire lock: {0}")]
    Lock(String),
    /// INCLUDERC/SWITCHRC nesting too deep.
    #[error("INCLUDERC recursion depth exceeded")]
    RecursionLimit,
}

/// Result of engine operations.
pub type EngineResult<T> = Result<T, EngineError>;

/// Mutable state during recipe processing.
#[derive(Default)]
pub struct State {
    /// Last condition result (for A flag).
    pub last_cond: bool,
    /// Last action succeeded (for a flag).
    pub last_succ: bool,
    /// Previous condition (for E flag).
    pub prev_cond: bool,
    /// Current INCLUDERC/SWITCHRC recursion depth.
    pub depth: usize,
}

/// Result of evaluating a single condition.
struct ConditionResult {
    /// Whether the condition matched.
    matched: bool,
    /// Score delta (nonzero only when scored).
    score: f64,
    /// Whether this was a weighted (scoring) condition.
    scored: bool,
}

impl ConditionResult {
    fn simple(matched: bool) -> Self {
        Self {
            matched,
            score: 0.0,
            scored: false,
        }
    }

    fn scored(score: f64) -> Self {
        Self {
            matched: true,
            score,
            scored: true,
        }
    }
}

/// Iterative weighted scoring matching procmail's misc.c:537-560.
fn score_regex(m: &Matcher, text: &str, wt: Weight) -> f64 {
    let mut score = 0.0;
    let mut w = wt.w;
    let mut ow = w * w;
    let mut pos = 0;

    while w != 0.0 && score > MIN32 && score < MAX32 {
        let Some((start, end)) = m.find_from(text, pos) else {
            break;
        };
        score += w;
        w *= wt.x;
        if end == start {
            // Zero-width match
            if wt.x > 0.0 && wt.x < 1.0 {
                score += w / (1.0 - wt.x);
            } else if wt.x >= 1.0 && w != 0.0 {
                score += if w < 0.0 { MIN32 } else { MAX32 };
            }
            break;
        }
        let nw = w * w;
        if nw < ow && ow < 1.0 {
            break;
        }
        ow = nw;
        pos = end;
        if pos > text.len() {
            break;
        }
    }
    score.clamp(MIN32, MAX32)
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

/// Recipe evaluation engine.
pub struct Engine {
    env: Environment,
    ctx: SubstCtx,
    verbose: bool,
    /// Kept alive so the fd backing stderr remains valid.
    logfile: Option<File>,
    namer: Namer,
    /// Whether EXITCODE was explicitly assigned.
    exit_was_set: bool,
    /// Real hostname for HOST mismatch detection.
    real_host: String,
    /// Global lockfile held while LOCKFILE is set.
    globlock: Option<FileLock>,
    /// Signal to stop processing current rcfile (HOST mismatch).
    abort: bool,
}

impl Engine {
    /// Create an engine with the given environment and substitution context.
    pub fn new(env: Environment, ctx: SubstCtx) -> Self {
        let real_host = env.get(VAR_HOST).unwrap_or("").to_owned();
        Self {
            env,
            ctx,
            verbose: false,
            logfile: None,
            namer: Namer::new(),
            exit_was_set: false,
            real_host,
            globlock: None,
            abort: false,
        }
    }

    /// Whether EXITCODE was explicitly assigned in an rcfile.
    pub fn exit_was_set(&self) -> bool {
        self.exit_was_set
    }

    /// Borrow the maildir unique-name generator.
    pub fn namer(&mut self) -> &mut Namer {
        &mut self.namer
    }

    /// Enable or disable verbose logging.
    pub fn set_verbose(&mut self, v: bool) {
        self.verbose = v;
    }

    /// Look up a variable by name.
    pub fn get_var(&self, name: &str) -> Option<&str> {
        self.env.get(name)
    }

    /// Look up a numeric variable, parsing via `value_as_int`.
    pub fn get_var_as_num(&self, name: &str, def: i64) -> i64 {
        match self.env.get(name) {
            Some(v) => crate::variables::value_as_int(v, def),
            None => def,
        }
    }

    /// Set a variable and apply any side effects.
    pub fn set_var(&mut self, name: &str, value: &str) {
        self.env.set(name, value);
        self.apply_side_effect(name, value);
    }

    fn apply_side_effect(&mut self, name: &str, value: &str) {
        match name {
            VAR_VERBOSE => {
                self.verbose = crate::variables::value_is_true(value);
            }
            VAR_UMASK => {
                if let Ok(m) = u32::from_str_radix(value, 8) {
                    stat::umask(Mode::from_bits_truncate(m));
                }
            }
            VAR_MAILDIR => {
                if let Err(e) = env::set_current_dir(value) {
                    eprintln!("can't chdir to {:?}: {}", value, e);
                    let cur = env::current_dir()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| ".".into());
                    self.env.set(name, &cur);
                }
            }
            VAR_EXITCODE => {
                self.exit_was_set = true;
            }
            VAR_SHIFT => {
                let n = value_as_int(value, 0).max(0) as usize;
                let drain = n.min(self.ctx.argv.len());
                self.ctx.argv.drain(..drain);
            }
            VAR_HOST => {
                if value != self.real_host {
                    eprintln!(
                        "HOST mismatch: \"{}\" strstrstr \"{}\"",
                        self.real_host, value,
                    );
                    self.abort = true;
                }
                self.env.set(VAR_HOST, self.real_host.clone());
            }
            VAR_LOCKFILE => self.set_globlock(value),
            VAR_LOG => {
                eprint!("{value}");
            }
            VAR_LOGFILE => {
                self.open_logfile(value);
            }
            _ => {}
        }
    }

    /// Open a logfile and redirect stderr to it.
    fn open_logfile(&mut self, path: &str) {
        if path.is_empty() {
            self.logfile = None;
            return;
        }
        let Ok(f) = OpenOptions::new().create(true).append(true).open(path)
        else {
            eprintln!("failed to open logfile: {}", path);
            return;
        };
        if dup2(f.as_raw_fd(), io::stderr().as_raw_fd()).is_err() {
            eprintln!("failed to redirect stderr to logfile: {}", path);
            return;
        }
        self.logfile = Some(f);
    }

    /// Acquire or release the global lockfile.
    fn set_globlock(&mut self, path: &str) {
        self.globlock = None;
        if path.is_empty() {
            return;
        }
        let timeout =
            self.get_var_as_num(VAR_LOCKTIMEOUT, DEF_LOCKTIMEOUT) as u64;
        let sleep = self.get_var_as_num(VAR_LOCKSLEEP, DEF_LOCKSLEEP) as u64;
        match FileLock::acquire_temp_retry(Path::new(path), timeout, sleep) {
            Ok(lock) => self.globlock = Some(lock),
            Err(e) => {
                eprintln!("failed to lock \"{}\": {}", path, e);
                self.env.remove(VAR_LOCKFILE);
            }
        }
    }

    /// Expand variables in a string.
    fn expand(&self, s: &str) -> String {
        crate::variables::subst(s, &self.ctx, &self.env)
    }

    /// Return a copy of `cond` with all string fields expanded.
    fn expand_condition(&self, cond: &Condition) -> Condition {
        match cond {
            Condition::Regex {
                pattern,
                negate,
                weight,
            } => Condition::Regex {
                pattern: self.expand(pattern),
                negate: *negate,
                weight: *weight,
            },
            Condition::Shell {
                cmd,
                negate,
                weight,
            } => Condition::Shell {
                cmd: self.expand(cmd),
                negate: *negate,
                weight: *weight,
            },
            Condition::Variable {
                name,
                pattern,
                weight,
            } => Condition::Variable {
                name: self.expand(name),
                pattern: self.expand(pattern),
                weight: *weight,
            },
            Condition::Size { .. } => cond.clone(),
            Condition::Subst { inner, negate } => Condition::Subst {
                inner: Box::new(self.expand_condition(inner)),
                negate: *negate,
            },
        }
    }

    /// Build a Command with a clean env from our Environment.
    fn spawn(&self, prog: &str) -> Command {
        let mut cmd = Command::new(prog);
        cmd.env_clear().envs(self.env.iter());
        cmd
    }

    /// Get text to grep based on H/B flags.
    fn grep_text<'a>(&self, msg: &'a Message, flags: &Flags) -> Cow<'a, str> {
        let bytes = match (flags.head, flags.body) {
            (true, true) => msg.as_bytes(),
            (true, false) => msg.header(),
            (false, true) => msg.body(),
            (false, false) => msg.header(),
        };
        String::from_utf8_lossy(bytes)
    }

    /// Get text for variable condition (VAR ?? pattern).
    fn get_variable_text<'a>(
        &'a self, name: &str, msg: &'a Message,
    ) -> Cow<'a, str> {
        match name {
            "H" => String::from_utf8_lossy(msg.header()),
            "B" => String::from_utf8_lossy(msg.body()),
            "HB" | "BH" => String::from_utf8_lossy(msg.as_bytes()),
            _ => Cow::Borrowed(self.get_var(name).unwrap_or("")),
        }
    }

    /// Create message for delivery based on h/b flags.
    fn message_for_delivery<'a>(
        &self, recipe: &Recipe, msg: &'a Message,
    ) -> Cow<'a, Message> {
        match (recipe.flags.pass_head, recipe.flags.pass_body) {
            (true, true) => Cow::Borrowed(msg),
            (true, false) => Cow::Owned(Message::from_parts(msg.header(), &[])),
            (false, true) => Cow::Owned(Message::from_parts(&[], msg.body())),
            (false, false) => Cow::Owned(Message::from_parts(&[], &[])),
        }
    }

    /// Run a shell command with message as stdin. Returns exit code.
    fn run_shell(&mut self, cmd: &str, input: &[u8]) -> EngineResult<i32> {
        let shell = self.get_var(VAR_SHELL).unwrap_or(DEF_SHELL).to_owned();
        let flags = self
            .get_var(VAR_SHELLFLAGS)
            .unwrap_or(DEF_SHELLFLAGS)
            .to_owned();
        let mut child = self
            .spawn(&shell)
            .arg(&flags)
            .arg(cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take()
            && let Err(e) = stdin.write_all(input)
            && e.kind() != ErrorKind::BrokenPipe
        {
            return Err(e.into());
        }

        let status =
            crate::util::wait_timeout(&mut child, self.timeout(), cmd)?;
        Ok(status.code().unwrap_or(-1))
    }

    fn timeout(&self) -> Duration {
        self.env.timeout()
    }

    fn eval_size(
        &self, op: Ordering, bytes: u64, weight: Option<Weight>, msg: &Message,
    ) -> EngineResult<ConditionResult> {
        let size = msg.len() as u64;
        let matched = match op {
            Ordering::Less => size < bytes,
            Ordering::Equal => size == bytes,
            Ordering::Greater => size > bytes,
        };

        if self.verbose {
            let sym = match op {
                Ordering::Less => '<',
                Ordering::Greater => '>',
                Ordering::Equal => '=',
            };
            eprintln!(
                "{} on {}{}",
                if matched { "Match" } else { "No match" },
                sym,
                bytes
            );
        }

        let Some(wt) = weight else {
            return Ok(ConditionResult::simple(matched));
        };

        // Weighted size: w * (M/L)^x or w * (L/M)^x
        let ratio = match op {
            Ordering::Less => bytes as f64 / size.max(1) as f64,
            Ordering::Equal => 1.0,
            Ordering::Greater => size as f64 / bytes.max(1) as f64,
        };
        let score = wt.w * ratio.powf(wt.x);
        Ok(ConditionResult::scored(score))
    }

    fn eval_shell(
        &mut self, cmd: &str, negate: bool, weight: Option<Weight>,
        recipe: &Recipe, msg: &Message,
    ) -> EngineResult<ConditionResult> {
        let text = self.grep_text(msg, &recipe.flags);
        let exit = self.run_shell(cmd, text.as_bytes())?;
        self.ctx.last_exit = exit;
        let ok = (exit == 0) ^ negate;

        if self.verbose {
            eprintln!("{} on ?{}", if ok { "Match" } else { "No match" }, cmd);
        }

        let Some(wt) = weight else {
            return Ok(ConditionResult::simple(ok));
        };

        // Procmail misc.c:577-591
        let mut score = 0.0;
        let mut w = wt.w;
        if negate {
            for _ in 0..exit {
                score += w;
                w *= wt.x;
                if score <= MIN32 || score >= MAX32 {
                    break;
                }
            }
        } else {
            score += if exit != 0 { wt.x } else { wt.w };
        }
        Ok(ConditionResult::scored(score))
    }

    /// Match a pattern against text, handling MATCH capture and weighting.
    fn eval_pattern(
        &mut self, text: &str, pattern: &str, negate: bool,
        weight: Option<Weight>, case_insens: bool, label: &str,
    ) -> EngineResult<ConditionResult> {
        let matcher = Matcher::new(pattern, case_insens)?;

        let result = matcher.exec(text);
        if result.matched
            && let Some(cap) = result.capture
        {
            self.set_var("MATCH", cap);
        }

        let Some(wt) = weight else {
            let matched = result.matched ^ negate;
            if self.verbose {
                eprintln!(
                    "{} on {}",
                    if matched { "Match" } else { "No match" },
                    label
                );
            }
            return Ok(ConditionResult::simple(matched));
        };

        let score = if negate {
            if result.matched { 0.0 } else { wt.w }
        } else {
            score_regex(&matcher, text, wt)
        };

        if self.verbose {
            eprintln!("Score {} on {}", score, label);
        }

        Ok(ConditionResult::scored(score))
    }

    fn eval_regex(
        &mut self, pattern: &str, negate: bool, weight: Option<Weight>,
        recipe: &Recipe, msg: &Message,
    ) -> EngineResult<ConditionResult> {
        let text = self.grep_text(msg, &recipe.flags);
        let label = format!("\"{}\"", pattern);
        self.eval_pattern(
            &text,
            pattern,
            negate,
            weight,
            !recipe.flags.case,
            &label,
        )
    }

    fn eval_variable(
        &mut self, name: &str, pattern: &str, weight: Option<Weight>,
        recipe: &Recipe, msg: &Message,
    ) -> EngineResult<ConditionResult> {
        let text = self.get_variable_text(name, msg).into_owned();
        let label = format!("{} ?? {}", name, pattern);
        self.eval_pattern(
            &text,
            pattern,
            false,
            weight,
            !recipe.flags.case,
            &label,
        )
    }

    /// Evaluate a single condition. Returns (matched, score_delta, is_scored).
    fn eval_condition(
        &mut self, cond: &Condition, recipe: &Recipe, msg: &Message,
    ) -> EngineResult<ConditionResult> {
        match cond {
            Condition::Regex {
                pattern,
                negate,
                weight,
            } => self.eval_regex(pattern, *negate, *weight, recipe, msg),
            Condition::Size { op, bytes, weight } => {
                self.eval_size(*op, *bytes, *weight, msg)
            }
            Condition::Shell {
                cmd,
                negate,
                weight,
            } => self.eval_shell(cmd, *negate, *weight, recipe, msg),
            Condition::Variable {
                name,
                pattern,
                weight,
            } => self.eval_variable(name, pattern, *weight, recipe, msg),
            Condition::Subst { inner, negate } => {
                let expanded = self.expand_condition(inner);
                let mut r = self.eval_condition(&expanded, recipe, msg)?;
                r.matched ^= negate;
                Ok(r)
            }
        }
    }

    /// Evaluate all conditions, returns (matched, score).
    fn eval_conditions(
        &mut self, recipe: &Recipe, msg: &Message,
    ) -> EngineResult<(bool, f64)> {
        if recipe.conds.is_empty() {
            return Ok((true, 0.0));
        }

        let mut score = 0.0f64;
        let mut has_score = false;

        for cond in &recipe.conds {
            let r = self.eval_condition(cond, recipe, msg)?;
            if !r.matched {
                return Ok((false, score));
            } else if r.scored {
                score += r.score;
                has_score = true;
            }
        }

        // misc.c:622-625: underflow forces match failure, overflow clamps
        if score <= MIN32 {
            return Ok((false, score));
        }
        if score > MAX32 {
            score = MAX32;
        }

        // If we used scoring, match requires score > 0
        Ok((!has_score || score > 0.0, score))
    }

    /// Check if chain flags allow this recipe to run.
    fn check_chain_flags(&self, flags: &Flags, state: &State) -> bool {
        // only if previous condition matched
        if flags.chain && !state.last_cond {
            return false;
        }
        // only if previous condition matched AND action succeeded
        if flags.succ && !(state.last_cond && state.last_succ) {
            return false;
        }
        // only if previous condition did NOT match
        if flags.r#else && state.prev_cond {
            return false;
        }
        // only if previous condition matched but action failed
        if flags.err && (!state.prev_cond || state.last_succ) {
            return false;
        }
        true
    }

    /// Resolve the lockfile path for a recipe.
    fn resolve_lockfile(&self, recipe: &Recipe) -> Option<String> {
        let lock = recipe.lockfile.as_ref()?;
        if lock.is_empty() {
            match &recipe.action {
                Action::Folder(paths) => {
                    let path_str = paths[0].to_string_lossy();
                    let expanded = self.expand(&path_str);
                    let (ft, _) = FolderType::parse(&expanded);
                    if !ft.needs_lock() {
                        return None;
                    }
                    let ext = self.get_var(VAR_LOCKEXT).unwrap_or(DEF_LOCKEXT);
                    Some(expanded + ext)
                }
                _ => None,
            }
        } else {
            Some(self.expand(lock))
        }
    }

    /// Hard-link the primary delivery file into secondary folders.
    fn deliver_secondaries(
        &mut self, paths: &[String], src: &str, primary_ft: FolderType,
        folder: &mut String,
    ) {
        if paths.is_empty() {
            return;
        }
        if primary_ft == FolderType::File {
            for sec in paths {
                eprintln!("Skipped \"{}\": can't link from mbox", sec);
            }
            return;
        }
        let src = Path::new(src);
        for sec in paths {
            let (ft, dir) = FolderType::parse(sec);
            if ft == FolderType::File {
                eprintln!("Skipped \"{}\"", sec);
                continue;
            }
            match delivery::link_secondary(
                src,
                Path::new(dir),
                ft,
                &mut self.namer,
            ) {
                Ok(p) => {
                    delivery::update_perms(Path::new(dir));
                    folder.push(' ');
                    folder.push_str(&p);
                }
                Err(e) => {
                    eprintln!("Couldn't make link to \"{}\": {}", sec, e);
                }
            }
        }
    }

    /// Deliver to a folder (mbox, maildir, or MH).
    /// Multiple paths are not supported for mbox; secondary paths are
    /// hard-linked from the file created by primary directory delivery.
    fn deliver_to_folder(
        &mut self, paths: &[String], recipe: &Recipe, msg: &Message,
    ) -> EngineResult<Outcome> {
        let (ft, path) = FolderType::parse(&paths[0]);
        let msg = self.message_for_delivery(recipe, msg);
        let opts = DeliveryOpts {
            raw: recipe.flags.raw,
        };

        let sender = msg.envelope_sender().unwrap_or("MAILER-DAEMON");
        let result =
            ft.deliver(Path::new(path), &msg, sender, opts, &mut self.namer);

        let result = match result {
            Ok(r) => r,
            Err(e) if recipe.flags.ignore => {
                eprintln!("Ignoring delivery error: {}", e);
                return Ok(Outcome::Continue);
            }
            Err(e) => return Err(e.into()),
        };

        delivery::update_perms(Path::new(path));

        let mut folder = result.path.clone();
        self.deliver_secondaries(&paths[1..], &result.path, ft, &mut folder);

        self.set_var("LASTFOLDER", &folder);
        self.ctx.lastfolder = folder.clone();

        if self.verbose {
            eprintln!("Delivered to {}", folder);
        }

        Ok(Outcome::Delivered(folder))
    }

    /// Deliver to a pipe command.
    fn deliver_to_pipe(
        &mut self, cmd: &str, recipe: &Recipe, msg: &mut Message,
        capture: Option<&str>,
    ) -> EngineResult<Outcome> {
        let delivery_msg = self.message_for_delivery(recipe, msg);
        let filter = recipe.flags.filter;
        let wait = recipe.flags.wait;
        let quiet = recipe.flags.quiet;

        let result = delivery::pipe(
            cmd,
            &delivery_msg,
            filter,
            wait,
            capture.is_some(),
            &self.env,
        );

        // Handle pipe errors based on w/W flags
        let result = match result {
            Ok(r) => r,
            Err(DeliveryError::PipeExit(code)) if wait => {
                self.ctx.last_exit = code;
                if !quiet {
                    eprintln!("Program failure ({}) of \"{}\"", code, cmd);
                }
                return Ok(Outcome::Default);
            }
            Err(DeliveryError::PipeSignal(sig)) if wait => {
                if !quiet {
                    eprintln!(
                        "Program terminated by signal {} \"{}\"",
                        sig, cmd
                    );
                }
                return Ok(Outcome::Default);
            }
            Err(e) => return Err(e.into()),
        };

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
            eprintln!("Piped to \"{}\"", cmd);
        }

        Ok(Outcome::Delivered(cmd.to_string()))
    }

    /// Forward to addresses.
    fn forward(
        &mut self, recipe: &Recipe, msg: &Message, addrs: &[String],
    ) -> EngineResult<Outcome> {
        let sendmail = self
            .get_var(VAR_SENDMAIL)
            .unwrap_or(DEF_SENDMAIL)
            .to_owned();
        let flags = self
            .get_var(VAR_SENDMAILFLAGS)
            .unwrap_or(DEF_SENDMAILFLAGS)
            .to_owned();
        let msg = self.message_for_delivery(recipe, msg);

        // Skip From_ line for forwarding
        let data = skip_from_line(msg.as_bytes());

        let flag_args: Vec<&str> = flags.split_whitespace().collect();
        let mut child = self
            .spawn(&sendmail)
            .args(&flag_args)
            .args(addrs)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take()
            && let Err(e) = stdin.write_all(data)
            && e.kind() != ErrorKind::BrokenPipe
        {
            return Err(e.into());
        }

        let status =
            crate::util::wait_timeout(&mut child, self.timeout(), &sendmail)?;
        self.ctx.last_exit = status.code().unwrap_or(-1);

        if !status.success() {
            return Err(DeliveryError::PipeExit(self.ctx.last_exit).into());
        }

        let dest = addrs.join(" ");
        self.set_var("LASTFOLDER", &dest);
        self.ctx.lastfolder = dest.clone();

        if self.verbose {
            eprintln!("Forwarded to {}", dest);
        }

        Ok(Outcome::Delivered(dest))
    }

    /// Perform the recipe's action.
    fn perform_action(
        &mut self, recipe: &Recipe, msg: &mut Message, state: &mut State,
    ) -> EngineResult<Outcome> {
        // Acquire lockfile if specified
        let _lock = if let Some(p) = self.resolve_lockfile(recipe) {
            let timeout =
                self.get_var_as_num(VAR_LOCKTIMEOUT, DEF_LOCKTIMEOUT) as u64;
            let sleep =
                self.get_var_as_num(VAR_LOCKSLEEP, DEF_LOCKSLEEP) as u64;
            Some(
                FileLock::acquire_temp_retry(Path::new(&p), timeout, sleep)
                    .map_err(|e| {
                        eprintln!("failed to acquire lock {}: {}", p, e);
                        EngineError::Lock(p)
                    })?,
            )
        } else {
            None
        };

        match &recipe.action {
            Action::Folder(paths) => {
                let expanded: Vec<_> = paths
                    .iter()
                    .map(|p| self.expand(&p.to_string_lossy()))
                    .collect();
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

    /// Evaluate a single recipe.
    fn eval_recipe(
        &mut self, recipe: &Recipe, msg: &mut Message, state: &mut State,
    ) -> EngineResult<Outcome> {
        if !self.check_chain_flags(&recipe.flags, state) {
            return Ok(Outcome::Default);
        }

        let (matched, score) = self.eval_conditions(recipe, msg)?;

        // Update state for next recipe
        if !recipe.flags.chain && !recipe.flags.succ {
            state.last_cond = matched;
        }
        if !recipe.flags.r#else {
            state.prev_cond = matched;
        }

        if !matched {
            return Ok(Outcome::Default);
        }

        let result = self.perform_action(recipe, msg, state)?;

        state.last_succ =
            matches!(result, Outcome::Delivered(_) | Outcome::Continue);
        // misc.c:647-648: if score truncates to 0 but was positive, use 1
        self.ctx.last_score = if score as i64 == 0 && score > 0.0 {
            1
        } else {
            score as i64
        };

        // Copy flag means continue processing even after delivery
        if recipe.flags.copy && matches!(result, Outcome::Delivered(_)) {
            return Ok(Outcome::Continue);
        }

        Ok(result)
    }

    /// Load and parse an rcfile. Returns None if file doesn't exist or fails
    /// to parse.
    fn load_rcfile(&self, path: &str) -> Option<Vec<Item>> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                if e.kind() != ErrorKind::NotFound && path != DEV_NULL {
                    eprintln!("failed to read rcfile {}: {}", path, e);
                }
                return None;
            }
        };

        match crate::config::parse(&content) {
            Ok(items) => Some(items),
            Err(e) => {
                eprintln!("failed to parse rcfile {}: {}", path, e);
                None
            }
        }
    }

    /// Load and run an rcfile. Returns `Some` if the caller should return.
    fn process_rcfile(
        &mut self, path: &str, var: &str, switch: bool, msg: &mut Message,
        state: &mut State,
    ) -> EngineResult<Option<Outcome>> {
        let expanded = self.expand(path);
        self.set_var(var, &expanded);
        if state.depth >= MAX_INCLUDE_DEPTH {
            return Err(EngineError::RecursionLimit);
        }
        let Some(items) = self.load_rcfile(&expanded) else {
            return Ok(None);
        };
        state.depth += 1;
        let outcome = self.process_items(&items, msg, state);
        state.depth -= 1;
        if switch {
            return outcome.map(Some);
        }
        match outcome {
            Ok(Outcome::Delivered(_)) => outcome.map(Some),
            other => {
                other?;
                Ok(None)
            }
        }
    }

    fn process_items(
        &mut self, items: &[Item], msg: &mut Message, state: &mut State,
    ) -> EngineResult<Outcome> {
        for item in items {
            match item {
                Item::Assign { name, value } => {
                    let expanded = self.expand(value);
                    self.set_var(name, &expanded);
                    if self.abort {
                        self.abort = false;
                        return Ok(Outcome::Default);
                    }
                }
                Item::Recipe(recipe) => {
                    let outcome = self.eval_recipe(recipe, msg, state)?;
                    match outcome {
                        Outcome::Delivered(_) => return Ok(outcome),
                        Outcome::Continue | Outcome::Default => {}
                    }
                }
                Item::Include(path) => {
                    if let Some(o) = self.process_rcfile(
                        path,
                        "INCLUDERC",
                        false,
                        msg,
                        state,
                    )? {
                        return Ok(o);
                    }
                }
                Item::Switch(path) => {
                    if path.is_empty() {
                        return Ok(Outcome::Default);
                    }
                    if let Some(o) =
                        self.process_rcfile(path, "SWITCHRC", true, msg, state)?
                    {
                        return Ok(o);
                    }
                }
            }
        }
        Ok(Outcome::Default)
    }

    /// Process a list of items (rcfile contents).
    pub fn process(
        &mut self, items: &[Item], msg: &mut Message,
    ) -> EngineResult<Outcome> {
        let mut state = State::default();
        self.process_items(items, msg, &mut state)
    }

    /// Execute TRAP command before exit (pipes.c:288-312).
    pub fn run_trap(&mut self, msg: &Message) {
        let trap = match self.get_var(VAR_TRAP) {
            Some(t) if !t.is_empty() => t.to_owned(),
            _ => return,
        };
        let user_set = self.get_var(VAR_EXITCODE).is_some();
        if !user_set {
            self.set_var(VAR_EXITCODE, "0");
        }
        let cmd = self.expand(&trap);
        let shell = self.get_var(VAR_SHELL).unwrap_or(DEF_SHELL).to_owned();
        let child = self
            .spawn(&shell)
            .arg(DEF_SHELLFLAGS)
            .arg(&cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn();
        let Ok(mut child) = child else { return };
        if let Some(mut w) = child.stdin.take() {
            let _ = w.write_all(msg.as_bytes());
        }
        let timeout = self.timeout();
        if let Ok(status) = crate::util::wait_timeout(&mut child, timeout, &cmd)
        {
            let code = status.code().unwrap_or(-1);
            if !user_set && code != 0 {
                self.set_var(VAR_EXITCODE, &code.to_string());
            }
        }
    }
}
