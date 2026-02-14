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
    pub fn new() -> Self {
        Self::default()
    }

    /// Copy all current process env vars into a new Environment.
    pub fn from_process() -> Self {
        Self {
            vars: env::vars().collect(),
        }
    }

    pub fn get<T>(&self, name: &T) -> Option<&str>
    where
        T: Borrow<str> + ?Sized,
    {
        self.vars.get(name.borrow()).map(|s| s.as_str())
    }

    pub fn set<T, U>(&mut self, name: T, value: U)
    where
        T: Into<String>,
        U: Into<String>,
    {
        self.vars.insert(name.into(), value.into());
    }

    pub fn remove(&mut self, name: &str) {
        self.vars.remove(name);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.vars.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn timeout(&self) -> Duration {
        let secs = self
            .get(VAR_TIMEOUT)
            .map(|v| value_as_int(v, DEF_TIMEOUT))
            .unwrap_or(DEF_TIMEOUT) as u64;
        Duration::from_secs(secs)
    }
}
