// Port of internal/utils/find_tsconfig.go: assign each linted file to the tsconfig whose program
// contains it (nearest config, its project references, then ancestor configs), like tsgo's
// findOrCreateDefaultConfiguredProjectForOpenScriptInfo.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

use rustc_hash::{FxHashMap, FxHashSet};
use tsrs_core::tspath::{self, Path};
use tsrs_core::CompilerOptions;
use tsrs_core::P;
use tsrs_tsoptions::{
    self as tsoptions, ExtendedConfigCacheEntry, ParseConfigHost, ParsedCommandLine,
};
use tsrs_vfs::FS;

pub struct ParseHost {
    pub fs: Arc<dyn FS>,
    pub cwd: String,
}

impl ParseConfigHost for ParseHost {
    fn fs(&self) -> &dyn FS {
        &*self.fs
    }
    fn get_current_directory(&self) -> &str {
        &self.cwd
    }
}

/// Permanent extended-config cache (tsrs_cli's ExtendedConfigCache).
#[derive(Default)]
pub struct ExtendedConfigCache {
    m: Mutex<FxHashMap<Path, P<ExtendedConfigCacheEntry>>>,
}

impl tsoptions::ExtendedConfigCache for ExtendedConfigCache {
    fn get_extended_config(
        &self,
        file_name: &str,
        path: &Path,
        resolution_stack: &[Path],
        host: &'static dyn ParseConfigHost,
    ) -> P<ExtendedConfigCacheEntry> {
        if let Some(entry) = self.m.lock().unwrap().get(path) {
            return *entry;
        }
        let entry = tsoptions::parse_extended_config(
            file_name,
            path.clone(),
            resolution_stack,
            host,
            Some(self),
        );
        *self.m.lock().unwrap().entry(path.clone()).or_insert(entry)
    }
}

pub struct ParsedConfig {
    pub config: Option<P<ParsedCommandLine>>,
    pub errors: Vec<P<tsrs_ast::Diagnostic>>,
}

pub struct TsConfigResolver {
    pub host: &'static ParseHost,
    pub ext_cache: &'static ExtendedConfigCache,
    configs: Mutex<FxHashMap<String, Arc<OnceLock<Arc<ParsedConfig>>>>>,
}

impl TsConfigResolver {
    pub fn new(
        host: &'static ParseHost,
        ext_cache: &'static ExtendedConfigCache,
    ) -> TsConfigResolver {
        TsConfigResolver {
            host,
            ext_cache,
            configs: Mutex::new(FxHashMap::default()),
        }
    }

    fn fs(&self) -> &dyn FS {
        &*self.host.fs
    }

    fn to_path(&self, file_name: &str) -> Path {
        tspath::to_path(
            file_name,
            &self.host.cwd,
            self.fs().use_case_sensitive_file_names(),
        )
    }

    /// configfileregistrybuilder.go ComputeConfigFileName (no custom config file name).
    pub fn compute_config_file_name(
        &self,
        file_name: &str,
        skip_search_in_directory_of_file: bool,
    ) -> String {
        let search_path = tspath::get_directory_path(file_name);
        let mut skip_tsconfig = skip_search_in_directory_of_file;
        let mut skip_jsconfig =
            skip_search_in_directory_of_file && !file_name.ends_with("/tsconfig.json");
        tspath::for_each_ancestor_directory(&search_path, |directory| {
            if !skip_tsconfig {
                let p = tspath::combine_paths(directory, &["tsconfig.json"]);
                if self.fs().file_exists(&p) {
                    return Some(p);
                }
            }
            if !skip_jsconfig {
                let p = tspath::combine_paths(directory, &["jsconfig.json"]);
                if self.fs().file_exists(&p) {
                    return Some(p);
                }
            }
            if directory.ends_with("/node_modules") {
                return Some(String::new());
            }
            skip_tsconfig = false;
            skip_jsconfig = false;
            None
        })
        .unwrap_or_default()
    }

    /// Parses (once) and returns a config file. Threads asking for a config that is being parsed wait for it:
    /// `resolve_all`'s threads all start on the same few configs, and each parse walks the include globs' whole
    /// directory tree.
    pub fn load(&self, config_file_name: &str) -> Arc<ParsedConfig> {
        let cell = self
            .configs
            .lock()
            .unwrap()
            .entry(config_file_name.to_string())
            .or_default()
            .clone();
        cell.get_or_init(|| {
            let (parsed, errors) = tsoptions::get_parsed_command_line_of_config_file(
                config_file_name,
                Some(&CompilerOptions {
                    checkers: crate::sched::checkers(),
                    ..Default::default()
                }),
                None,
                self.host,
                Some(self.ext_cache),
            );
            Arc::new(ParsedConfig {
                config: parsed.map(P::new),
                errors,
            })
        })
        .clone()
    }

    /// find_tsconfig.go findConfigWithReferences: breadth-first over the config and its project
    /// references for a config whose file list contains `path`; then the same from the next ancestor config.
    fn find_config_with_references(
        &self,
        file_name: &str,
        path: &Path,
        config_file_name: &str,
        visited: &mut FxHashSet<String>,
    ) -> String {
        let mut level: VecDeque<String> = VecDeque::from([config_file_name.to_string()]);
        while !level.is_empty() {
            let mut next: Vec<String> = Vec::new();
            for node in level.drain(..) {
                if !visited.insert(node.clone()) {
                    continue;
                }
                let parsed = self.load(&node);
                let Some(config) = parsed.config else {
                    continue;
                };
                if !config.file_names().is_empty() && config.file_names_by_path().contains_key(path)
                {
                    return node;
                }
                for r in config.resolved_project_reference_paths() {
                    if !next.contains(r) {
                        next.push(r.clone());
                    }
                }
            }
            level.extend(next);
        }
        let disable_solution_searching = self
            .load(config_file_name)
            .config
            .and_then(|c| c.compiler_options())
            .is_some_and(|o| o.disable_solution_searching.is_true());
        if disable_solution_searching {
            return String::new();
        }
        let ancestor = self.compute_config_file_name(config_file_name, true);
        if !ancestor.is_empty() {
            return self.find_config_with_references(file_name, path, &ancestor, visited);
        }
        String::new()
    }

    /// Returns the tsconfig for `file`, or "" when no configured program contains it.
    pub fn resolve(&self, file: &str) -> String {
        let config = self.compute_config_file_name(file, false);
        if config.is_empty() {
            return String::new();
        }
        let path = self.to_path(file);
        let mut visited = FxHashSet::default();
        self.find_config_with_references(file, &path, &config, &mut visited)
    }

    pub fn resolve_all(&self, files: &[String], threads: usize) -> Vec<String> {
        let next = std::sync::atomic::AtomicUsize::new(0);
        let results: Vec<Mutex<String>> = files.iter().map(|_| Mutex::new(String::new())).collect();
        std::thread::scope(|s| {
            for _ in 0..threads.max(1) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= files.len() {
                        break;
                    }
                    *results[i].lock().unwrap() = self.resolve(&files[i]);
                });
            }
        });
        results
            .into_iter()
            .map(|m| m.into_inner().unwrap())
            .collect()
    }
}
