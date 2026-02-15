//! Recipe evaluation engine.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::mem;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use nix::sys::stat::{self, Mode};
use nix::unistd::dup2;

use regex::RegexBuilder;

use crate::config::{Action, Condition, Flags, HeaderOp, Item, Recipe, Weight};
use crate::delivery::{self, DeliveryError, DeliveryOpts, FolderType, Namer};
use crate::field::{self, Field};
use crate::locking::FileLock;
use crate::mail::Message;
use crate::re::Matcher;
use crate::util::wait_timeout;
use crate::variables::{
    BacktickFn, DEF_LINEBUF, DEV_NULL, Environment, HOST, LINEBUF, LOCKEXT,
    LOCKSLEEP, LOCKTIMEOUT, LOGABSTRACT, MIN_LINEBUF, SENDMAIL, SENDMAILFLAGS,
    SHELL, SHELLFLAGS, SubstCtx, VAR_EXITCODE, VAR_HOST, VAR_INCLUDERC,
    VAR_LASTFOLDER, VAR_LINEBUF, VAR_LOCKFILE, VAR_LOG, VAR_LOGFILE,
    VAR_MAILDIR, VAR_MATCH, VAR_PROCMAIL_OVERFLOW, VAR_SHIFT, VAR_SWITCHRC,
    VAR_TRAP, VAR_UMASK, VAR_VERBOSE, is_builtin, subst_limited, value_as_int,
};

#[cfg(test)]
mod tests;

const MAX_INCLUDE_DEPTH: usize = 32;
const MAX32: f64 = i32::MAX as f64;
const MIN32: f64 = i32::MIN as f64;
const MAX_SUBJECT: usize = 78;
const MAX_FOLDER: usize = 61;
const TAB_STOP: usize = 72;
const TAB: usize = 8;
const LOGABSTRACT_ALL: i64 = 2;

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

    let bytes = text.as_bytes();
    while w != 0.0 && score > MIN32 && score < MAX32 && pos < text.len() {
        let Some((start, end)) = m.find_from(text, pos) else {
            break;
        };
        if end == start {
            // Our regex engine doesn't consume \n with the match like
            // procmail's does.  A zero-width match at \n means the
            // regex stopped at a line boundary.  Skip past consecutive
            // \n bytes, counting each as a blank line except the first
            // (which is the line terminator for the preceding match).
            if bytes[pos] == b'\n' {
                let skip = if start > 0 { 1 } else { 0 };
                while pos < text.len() && bytes[pos] == b'\n' {
                    if pos >= start + skip {
                        score += w;
                        w *= wt.x;
                    }
                    pos += 1;
                }
                if pos == text.len() {
                    score += w;
                    w *= wt.x;
                }
                continue;
            }
            if wt.x > 0.0 && wt.x < 1.0 {
                score += w;
                w *= wt.x;
                score += w / (1.0 - wt.x);
            } else if wt.x >= 1.0 && w != 0.0 {
                score += w;
                w *= wt.x;
                score += if w < 0.0 { MIN32 } else { MAX32 };
            }
            break;
        }
        score += w;
        w *= wt.x;
        let nw = w * w;
        if nw < ow && ow < 1.0 {
            break;
        }
        ow = nw;
        pos = end;
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

/// Spawn `$SHELL $SHELLFLAGS cmd` with `input` on stdin, capture stdout,
/// strip trailing newlines (pipes.c:277).
fn run_backtick(
    shell: &str, flags: &str, cmd: &str, input: &[u8],
    envs: &[(String, String)], timeout: Duration,
) -> String {
    let child = Command::new(shell)
        .arg(flags)
        .arg(cmd)
        .env_clear()
        .envs(envs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        return String::new();
    };
    if let Some(mut w) = child.stdin.take() {
        let _ = w.write_all(input);
    }
    let mut buf = Vec::new();
    if let Some(mut r) = child.stdout.take() {
        let _ = io::Read::read_to_end(&mut r, &mut buf);
    }
    let _ = wait_timeout(&mut child, timeout, cmd);
    while buf.last() == Some(&b'\n') {
        buf.pop();
    }
    String::from_utf8_lossy(&buf).into_owned()
}

/// Recipe evaluation engine.
pub struct Engine {
    env: Environment,
    ctx: SubstCtx,
    verbose: bool,
    dryrun: bool,
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
    /// Current rcfile name for dry-run output.
    rcfile: String,
}

impl Engine {
    /// Create an engine with the given environment and substitution context.
    pub fn new(env: Environment, ctx: SubstCtx) -> Self {
        let real_host = env.get_or_default(&HOST).to_owned();
        Self {
            env,
            ctx,
            verbose: false,
            dryrun: false,
            logfile: None,
            namer: Namer::new(),
            exit_was_set: false,
            real_host,
            globlock: None,
            abort: false,
            rcfile: String::new(),
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

    /// Enable dry-run mode (no folder delivery or locking).
    pub fn set_dryrun(&mut self, v: bool) {
        self.dryrun = v;
    }

    /// Whether dry-run mode is active.
    pub fn dryrun(&self) -> bool {
        self.dryrun
    }

    /// Set the current rcfile name for dry-run output.
    pub fn set_rcfile(&mut self, name: &str) {
        self.rcfile = name.to_owned();
    }

    fn drylog(&self, line: usize, msg: &str) {
        eprintln!("[{}:{}] {}", self.rcfile, line, msg);
    }

    /// Look up a variable by name.
    pub fn get_var(&self, name: &str) -> Option<&str> {
        self.env.get(name)
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
            VAR_LINEBUF => {
                let n = value_as_int(value, DEF_LINEBUF as i64);
                if (n as usize) < MIN_LINEBUF {
                    self.env.set(name, MIN_LINEBUF.to_string());
                }
            }
            VAR_LOCKFILE => self.set_globlock(value),
            VAR_LOG => {
                eprint!("{value}");
            }
            VAR_LOGFILE => {
                if !self.dryrun {
                    self.open_logfile(value);
                }
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
        if path.is_empty() || self.dryrun {
            return;
        }
        let timeout = self.env.get_num(&LOCKTIMEOUT) as u64;
        let sleep = self.env.get_num(&LOCKSLEEP) as u64;
        match FileLock::acquire_temp_retry(Path::new(path), timeout, sleep) {
            Ok(lock) => self.globlock = Some(lock),
            Err(e) => {
                eprintln!("failed to lock \"{}\": {}", path, e);
                self.env.remove(VAR_LOCKFILE);
            }
        }
    }

    /// Expand variables (and backtick commands when `msg` is provided).
    fn expand(&mut self, s: &str, msg: Option<&Message>) -> String {
        let limit = self.env.get_num(&LINEBUF) as usize;
        let run;
        let runner: Option<BacktickFn>;
        if let Some(msg) = msg {
            let shell = self.env.get_or_default(&SHELL).to_owned();
            let flags = self.env.get_or_default(&SHELLFLAGS).to_owned();
            let envs: Vec<_> = self
                .env
                .iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect();
            let timeout = self.timeout();
            let input = msg.as_bytes().to_vec();
            run = move |cmd: &str| {
                run_backtick(&shell, &flags, cmd, &input, &envs, timeout)
            };
            runner = Some(&run as BacktickFn);
        } else {
            runner = None;
        }
        let (r, overflow) =
            subst_limited(&self.env, &self.ctx, s, limit, runner);
        if overflow {
            self.env.set(VAR_PROCMAIL_OVERFLOW, "yes");
        }
        r
    }

    /// Return a copy of `cond` with all string fields expanded.
    fn expand_condition(
        &mut self, cond: &Condition, msg: &Message,
    ) -> Condition {
        match cond {
            Condition::Regex {
                pattern,
                negate,
                weight,
            } => Condition::Regex {
                pattern: self.expand(pattern, Some(msg)),
                negate: *negate,
                weight: *weight,
            },
            Condition::Shell {
                cmd,
                negate,
                weight,
            } => Condition::Shell {
                cmd: self.expand(cmd, Some(msg)),
                negate: *negate,
                weight: *weight,
            },
            Condition::Variable {
                name,
                pattern,
                weight,
            } => Condition::Variable {
                name: self.expand(name, Some(msg)),
                pattern: self.expand(pattern, Some(msg)),
                weight: *weight,
            },
            Condition::Size { .. } => cond.clone(),
            Condition::Subst { inner, negate } => Condition::Subst {
                inner: Box::new(self.expand_condition(inner, msg)),
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
        let shell = self.env.get_or_default(&SHELL).to_owned();
        let flags = self.env.get_or_default(&SHELLFLAGS).to_owned();
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
        &self, op: Ordering, bytes: u64, negate: bool, weight: Option<Weight>,
        msg: &Message,
    ) -> EngineResult<ConditionResult> {
        let size = msg.len() as u64;
        let matched = match op {
            Ordering::Less => size < bytes,
            Ordering::Equal => size == bytes,
            Ordering::Greater => size > bytes,
        };
        let matched = matched ^ negate;

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
            self.set_var(VAR_MATCH, cap);
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
            Condition::Size {
                op,
                bytes,
                negate,
                weight,
            } => self.eval_size(*op, *bytes, *negate, *weight, msg),
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
                let expanded = self.expand_condition(inner, msg);
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
        let mut failed = false;

        for cond in &recipe.conds {
            let r = self.eval_condition(cond, recipe, msg)?;
            if !r.matched {
                failed = true;
            }
            if r.scored {
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

        Ok((!failed && (!has_score || score > 0.0), score))
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
    fn resolve_lockfile(
        &mut self, recipe: &Recipe, msg: &Message,
    ) -> Option<String> {
        let lock = recipe.lockfile.as_ref()?;
        if lock.is_empty() {
            match &recipe.action {
                Action::Folder(paths) => {
                    let path_str = paths[0].to_string_lossy();
                    let expanded = self.expand(&path_str, Some(msg));
                    let (ft, _) = FolderType::parse(&expanded);
                    if !ft.needs_lock() {
                        return None;
                    }
                    let ext = self.env.get_or_default(&LOCKEXT);
                    Some(expanded + ext)
                }
                _ => None,
            }
        } else {
            Some(self.expand(lock, Some(msg)))
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
        if self.dryrun {
            let folder = paths.join(" ");
            self.set_var(VAR_LASTFOLDER, &folder);
            self.ctx.lastfolder = folder.clone();
            return Ok(Outcome::Delivered(folder));
        }
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

        self.set_var(VAR_LASTFOLDER, &folder);
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
                return Ok(Outcome::Continue);
            }
            if let Some(var) = capture {
                let text = String::from_utf8_lossy(output);
                self.set_var(var, &text);
                return Ok(Outcome::Continue);
            }
        }

        self.set_var(VAR_LASTFOLDER, cmd);
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
        if self.dryrun {
            let dest = addrs.join(" ");
            self.set_var(VAR_LASTFOLDER, &dest);
            self.ctx.lastfolder = dest.clone();
            return Ok(Outcome::Delivered(dest));
        }
        let sendmail = self.env.get_or_default(&SENDMAIL).to_owned();
        let flags = self.env.get_or_default(&SENDMAILFLAGS).to_owned();
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
        self.set_var(VAR_LASTFOLDER, &dest);
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
        let _lock = if self.dryrun {
            None
        } else if let Some(p) = self.resolve_lockfile(recipe, msg) {
            if self.env.get(VAR_LOCKFILE).is_some_and(|gl| gl == p) {
                eprintln!("Deadlock attempted on \"{p}\"");
                None
            } else {
                let timeout = self.env.get_num(&LOCKTIMEOUT) as u64;
                let sleep = self.env.get_num(&LOCKSLEEP) as u64;
                Some(
                    FileLock::acquire_temp_retry(Path::new(&p), timeout, sleep)
                        .map_err(|e| {
                            eprintln!("failed to acquire lock {}: {}", p, e);
                            EngineError::Lock(p)
                        })?,
                )
            }
        } else {
            None
        };

        match &recipe.action {
            Action::Folder(paths) => {
                let expanded: Vec<_> = paths
                    .iter()
                    .map(|p| self.expand(&p.to_string_lossy(), Some(msg)))
                    .collect();
                self.deliver_to_folder(&expanded, recipe, msg)
            }
            Action::Pipe { cmd, capture } => {
                let expanded = self.expand(cmd, Some(msg));
                self.deliver_to_pipe(&expanded, recipe, msg, capture.as_deref())
            }
            Action::Forward(addrs) => {
                let expanded: Vec<_> =
                    addrs.iter().map(|a| self.expand(a, Some(msg))).collect();
                self.forward(recipe, msg, &expanded)
            }
            Action::Nested(items) => self.process_items(items, msg, state),
        }
    }

    /// Evaluate a single recipe.
    fn eval_recipe(
        &mut self, recipe: &Recipe, line: usize, msg: &mut Message,
        state: &mut State,
    ) -> EngineResult<Outcome> {
        if !self.check_chain_flags(&recipe.flags, state) {
            return Ok(Outcome::Default);
        }

        let (matched, score) = self.eval_conditions(recipe, msg)?;

        self.ctx.last_score = if score as i64 == 0 && score > 0.0 {
            1
        } else {
            score as i64
        };

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

        if self.dryrun {
            self.log_match(recipe, line);
        }

        let result = self.perform_action(recipe, msg, state)?;

        state.last_succ =
            matches!(result, Outcome::Delivered(_) | Outcome::Continue);

        // Copy flag means continue processing even after delivery
        if let Outcome::Delivered(ref f) = result
            && recipe.flags.copy
        {
            if self.env.get_num(&LOGABSTRACT) == LOGABSTRACT_ALL {
                let f = f.clone();
                self.log_abstract(&f, msg);
            }
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

        match crate::config::parse(&content, path) {
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
        let expanded = self.expand(path, Some(msg));
        self.set_var(var, &expanded);
        if state.depth >= MAX_INCLUDE_DEPTH {
            return Err(EngineError::RecursionLimit);
        }
        let Some(items) = self.load_rcfile(&expanded) else {
            return Ok(None);
        };
        let prev = mem::replace(&mut self.rcfile, expanded.clone());
        state.depth += 1;
        let outcome = self.process_items(&items, msg, state);
        state.depth -= 1;
        self.rcfile = prev;
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

    /// Apply `VAR =~ s/pat/rep/flags` substitution.
    fn apply_subst(
        &mut self, name: &str, pattern: &str, replace: &str, global: bool,
        case_insensitive: bool,
    ) {
        let pat = self.expand(pattern, None);
        let rep = self.expand(replace, None);
        let Ok(re) = RegexBuilder::new(&pat)
            .case_insensitive(case_insensitive)
            .build()
        else {
            eprintln!("bad regex in =~: {pat}");
            return;
        };
        let val = self.env.get_or_default(name).to_owned();
        let result = if global {
            re.replace_all(&val, rep.as_str())
        } else {
            re.replace(&val, rep.as_str())
        };
        self.set_var(name, &result);
    }

    /// Apply `@X Header: value` manipulation on the in-flight message.
    fn apply_header_op(&mut self, op: &HeaderOp, msg: &mut Message) {
        let (field, value) = match op {
            HeaderOp::DeleteInsert { field, value }
            | HeaderOp::RenameInsert { field, value }
            | HeaderOp::AddIfNot { field, value }
            | HeaderOp::AddAlways { field, value } => {
                (field.as_str(), Some(self.expand(value, None)))
            }
            HeaderOp::Delete { field } => (field.as_str(), None),
        };
        let pat = field.as_bytes();
        let mut fields = field::parse_bytes(msg.header());
        match op {
            HeaderOp::DeleteInsert { .. } => {
                fields.remove_all(pat);
                if let Some(v) = value
                    && !v.is_empty()
                {
                    fields.push(Field::from_parts(
                        field.as_bytes(),
                        v.as_bytes(),
                    ));
                }
            }
            HeaderOp::RenameInsert { .. } => {
                fields.prepend_old(pat);
                if let Some(v) = value {
                    fields.push(Field::from_parts(
                        field.as_bytes(),
                        v.as_bytes(),
                    ));
                }
            }
            HeaderOp::AddIfNot { .. } => {
                if fields.find(pat).is_none()
                    && let Some(v) = value
                {
                    fields.push(Field::from_parts(
                        field.as_bytes(),
                        v.as_bytes(),
                    ));
                }
            }
            HeaderOp::AddAlways { .. } => {
                if let Some(v) = value {
                    fields.push(Field::from_parts(
                        field.as_bytes(),
                        v.as_bytes(),
                    ));
                }
            }
            HeaderOp::Delete { .. } => {
                fields.remove_all(pat);
            }
        }
        let mut header = Vec::new();
        fields.write_to(&mut header).unwrap();
        *msg = Message::from_parts(&header, msg.body());
    }

    fn process_items(
        &mut self, items: &[Item], msg: &mut Message, state: &mut State,
    ) -> EngineResult<Outcome> {
        for item in items {
            match item {
                Item::Assign {
                    name, value, line, ..
                } => {
                    let expanded = self.expand(value, Some(msg));
                    if self.dryrun && !is_builtin(name) {
                        self.drylog(
                            *line,
                            &format!("assign: {}={:?}", name, expanded),
                        );
                    }
                    self.set_var(name, &expanded);
                    if self.abort {
                        self.abort = false;
                        return Ok(Outcome::Default);
                    }
                }
                Item::Subst {
                    name,
                    pattern,
                    replace,
                    global,
                    case_insensitive,
                    line,
                } => {
                    self.apply_subst(
                        name,
                        pattern,
                        replace,
                        *global,
                        *case_insensitive,
                    );
                    if self.dryrun {
                        let flags = match (*global, *case_insensitive) {
                            (true, true) => "gi",
                            (true, false) => "g",
                            (false, true) => "i",
                            (false, false) => "",
                        };
                        let val = self.get_var(name).unwrap_or("");
                        self.drylog(
                            *line,
                            &format!(
                                "subst: {} =~ s/{}/{}/{} -> {:?}",
                                name, pattern, replace, flags, val
                            ),
                        );
                    }
                }
                Item::HeaderOp { op, line } => {
                    if self.dryrun {
                        self.log_header_op(op, *line);
                    }
                    self.apply_header_op(op, msg);
                }
                Item::Recipe { recipe, line } => {
                    let outcome =
                        self.eval_recipe(recipe, *line, msg, state)?;
                    match outcome {
                        Outcome::Delivered(_) => return Ok(outcome),
                        Outcome::Continue | Outcome::Default => {}
                    }
                }
                Item::Include { path, .. } => {
                    if let Some(o) = self.process_rcfile(
                        path,
                        VAR_INCLUDERC,
                        false,
                        msg,
                        state,
                    )? {
                        return Ok(o);
                    }
                }
                Item::Switch { path, .. } => {
                    if path.is_empty() {
                        return Ok(Outcome::Default);
                    }
                    if let Some(o) = self.process_rcfile(
                        path,
                        VAR_SWITCHRC,
                        true,
                        msg,
                        state,
                    )? {
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

    /// Log the matched recipe action in dryrun mode.
    fn log_match(&mut self, recipe: &Recipe, line: usize) {
        let msg = match &recipe.action {
            Action::Folder(paths) => {
                let expanded: Vec<_> = paths
                    .iter()
                    .map(|p| self.expand(&p.to_string_lossy(), None))
                    .collect();
                format!("deliver: {}", expanded.join(" "))
            }
            Action::Pipe { cmd, capture } => {
                let expanded = self.expand(cmd, None);
                if let Some(var) = capture {
                    format!("capture: {}=| {}", var, expanded)
                } else if recipe.flags.filter {
                    format!("filter: {}", expanded)
                } else {
                    format!("pipe: {}", expanded)
                }
            }
            Action::Forward(addrs) => {
                let expanded: Vec<_> =
                    addrs.iter().map(|a| self.expand(a, None)).collect();
                format!("forward: {}", expanded.join(" "))
            }
            Action::Nested(_) => return,
        };
        self.drylog(line, &msg);
    }

    /// Log a header operation in dryrun mode.
    fn log_header_op(&mut self, op: &HeaderOp, line: usize) {
        let msg = match op {
            HeaderOp::DeleteInsert { field, value } => {
                let val = self.expand(value, None);
                format!("header: @I {}: {}", field, val)
            }
            HeaderOp::RenameInsert { field, value } => {
                let val = self.expand(value, None);
                format!("header: @i {}: {}", field, val)
            }
            HeaderOp::AddIfNot { field, value } => {
                let val = self.expand(value, None);
                format!("header: @a {}: {}", field, val)
            }
            HeaderOp::AddAlways { field, value } => {
                let val = self.expand(value, None);
                format!("header: @A {}: {}", field, val)
            }
            HeaderOp::Delete { field } => {
                format!("header: @D {}:", field)
            }
        };
        self.drylog(line, &msg);
    }

    /// Log delivery abstract (From_ line, Subject, Folder, size).
    pub fn log_abstract(&self, folder: &str, msg: &Message) {
        let la = self.env.get_num(&LOGABSTRACT);
        if !(la > 0 || (self.logfile.is_some() || self.verbose) && la != 0) {
            return;
        }

        if let Some(from) = msg.from_line() {
            eprintln!("{}", String::from_utf8_lossy(from));
        }

        if let Some(subj) = msg.get_header("Subject") {
            let subj: String = subj
                .chars()
                .map(|c| if c == '\t' { ' ' } else { c })
                .collect();
            let subj = if subj.len() > MAX_SUBJECT {
                &subj[..MAX_SUBJECT]
            } else {
                &subj
            };
            eprintln!(" Subject: {subj}");
        }

        let detabbed: String = folder
            .chars()
            .map(|c| if c.is_ascii_control() { ' ' } else { c })
            .take(MAX_FOLDER)
            .collect();
        let col = 10 + detabbed.len();
        let col = col - col % TAB;
        let tabs = TAB_STOP.saturating_sub(col).div_ceil(TAB);
        let pad: String = "\t".repeat(tabs.max(1));
        // mailfold.c:87,116-118: lastdump = message length, +1 for forced
        // trailing blank line if the message doesn't already end with \n\n
        let size = msg.len() + usize::from(!msg.as_bytes().ends_with(b"\n\n"));
        eprintln!("  Folder: {detabbed}{pad}{:>7}", size);
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
        let cmd = self.expand(&trap, Some(msg));
        let shell = self.env.get_or_default(&SHELL).to_owned();
        let child = self
            .spawn(&shell)
            .arg(SHELLFLAGS.def.unwrap())
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
