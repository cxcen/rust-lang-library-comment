use crate::collections::BTreeMap;
use crate::ffi::{OsStr, OsString};
use crate::sys::process::EnvKey;
use crate::{env, fmt};

/// 存储对某个环境所做的一组更改
#[derive(Clone, Default)]
pub struct CommandEnv {
    clear: bool,
    saw_path: bool,
    vars: BTreeMap<EnvKey, Option<OsString>>,
}

impl fmt::Debug for CommandEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug_command_env = f.debug_struct("CommandEnv");
        debug_command_env.field("clear", &self.clear).field("vars", &self.vars);
        debug_command_env.finish()
    }
}

impl CommandEnv {
    // 捕获当前环境，并应用这些更改
    pub fn capture(&self) -> BTreeMap<EnvKey, OsString> {
        let mut result = BTreeMap::<EnvKey, OsString>::new();
        if !self.clear {
            for (k, v) in env::vars_os() {
                result.insert(k.into(), v);
            }
        }
        for (k, maybe_v) in &self.vars {
            if let &Some(ref v) = maybe_v {
                result.insert(k.clone(), v.clone());
            } else {
                result.remove(k);
            }
        }
        result
    }

    pub fn is_unchanged(&self) -> bool {
        !self.clear && self.vars.is_empty()
    }

    pub fn capture_if_changed(&self) -> Option<BTreeMap<EnvKey, OsString>> {
        if self.is_unchanged() { None } else { Some(self.capture()) }
    }

    // 以下函数用于逐步构建更改
    pub fn set(&mut self, key: &OsStr, value: &OsStr) {
        let key = EnvKey::from(key);
        self.maybe_saw_path(&key);
        self.vars.insert(key, Some(value.to_owned()));
    }

    pub fn remove(&mut self, key: &OsStr) {
        let key = EnvKey::from(key);
        self.maybe_saw_path(&key);
        if self.clear {
            self.vars.remove(&key);
        } else {
            self.vars.insert(key, None);
        }
    }

    pub fn clear(&mut self) {
        self.clear = true;
        self.vars.clear();
    }

    pub fn does_clear(&self) -> bool {
        self.clear
    }

    pub fn have_changed_path(&self) -> bool {
        self.saw_path || self.clear
    }

    fn maybe_saw_path(&mut self, key: &EnvKey) {
        if !self.saw_path && key == "PATH" {
            self.saw_path = true;
        }
    }

    pub fn iter(&self) -> CommandEnvs<'_> {
        let iter = self.vars.iter();
        CommandEnvs { iter }
    }
}

#[derive(Debug)]
pub struct CommandEnvs<'a> {
    iter: crate::collections::btree_map::Iter<'a, EnvKey, Option<OsString>>,
}

impl<'a> Iterator for CommandEnvs<'a> {
    type Item = (&'a OsStr, Option<&'a OsStr>);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(key, value)| (key.as_ref(), value.as_deref()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a> ExactSizeIterator for CommandEnvs<'a> {
    fn len(&self) -> usize {
        self.iter.len()
    }
    fn is_empty(&self) -> bool {
        self.iter.is_empty()
    }
}
