use std::collections::HashMap;
use std::env;
use std::time::Duration;

use linker_set::set;

use super::{TIMEOUT, VarName, Variable, defaults, value_as_int};

/// Canonical variable store for all variable lookups.
#[derive(Default)]
pub struct Environment {
    vars: HashMap<String, String>,
}

impl Environment {
    /// Create an empty environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Copy all current process env vars into a new Environment.
    pub fn from_process() -> Self {
        Self {
            vars: env::vars().collect(),
        }
    }

    /// Look up a variable by name.
    pub fn get<T>(&self, v: &T) -> Option<&str>
    where
        T: VarName + ?Sized,
    {
        self.vars.get(v.name()).map(|s| s.as_str())
    }

    /// Returns value, or the variable's default, or "".
    pub fn get_or_default<T>(&self, v: &T) -> &str
    where
        T: VarName + ?Sized,
    {
        self.get(v).or_else(|| v.default()).unwrap_or("")
    }

    /// Parse as integer.  Uses variable's default if unset or unparseable.
    pub fn get_num<T>(&self, v: &T) -> i64
    where
        T: VarName + ?Sized,
    {
        let def = v.default().and_then(|d| d.parse::<i64>().ok()).unwrap_or(0);
        match self.get(v) {
            Some(s) => value_as_int(s, def),
            None => def,
        }
    }

    /// Set a variable, overwriting any previous value.
    pub fn set<T, U>(&mut self, name: T, value: U)
    where
        T: Into<String>,
        U: Into<String>,
    {
        self.vars.insert(name.into(), value.into());
    }

    /// Set variable to its default value.
    pub fn set_default(&mut self, v: &Variable) {
        self.set(v.name, v.def.unwrap_or(""));
    }

    /// Set all variables that have defaults from the linker set.
    pub fn set_all_defaults(&mut self) {
        for v in set!(defaults).iter() {
            self.set_default(v);
        }
    }

    /// Remove a variable.
    pub fn remove(&mut self, name: &str) {
        self.vars.remove(name);
    }

    /// Iterate over all variable name-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.vars.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Zero or negative means "no timeout", matching procmail's `alarm(0)`.
    pub fn timeout(&self) -> Duration {
        let secs = self.get_num(&TIMEOUT);
        if secs <= 0 {
            Duration::MAX
        } else {
            Duration::from_secs(secs as u64)
        }
    }
}
