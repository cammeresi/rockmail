use std::borrow::Borrow;
use std::collections::HashMap;
use std::env;
use std::time::Duration;

use super::{DEF_TIMEOUT, VAR_TIMEOUT, value_as_int};

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
    pub fn get<T>(&self, name: &T) -> Option<&str>
    where
        T: Borrow<str> + ?Sized,
    {
        self.vars.get(name.borrow()).map(|s| s.as_str())
    }

    /// Set a variable, overwriting any previous value.
    pub fn set<T, U>(&mut self, name: T, value: U)
    where
        T: Into<String>,
        U: Into<String>,
    {
        self.vars.insert(name.into(), value.into());
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
        let secs = self
            .get(VAR_TIMEOUT)
            .map(|v| value_as_int(v, DEF_TIMEOUT))
            .unwrap_or(DEF_TIMEOUT);
        if secs <= 0 {
            Duration::MAX
        } else {
            Duration::from_secs(secs as u64)
        }
    }
}
