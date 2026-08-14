//! Finds the Codex runtime bundled with ChatGPT/Codex Desktop on Windows.
//!
//! The resolver only inspects executable installation locations. It never
//! opens Codex Home, browser data, cookies, `auth.json`, or any token store.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSource {
    DesktopUserRuntime,
    DesktopInstall,
    PackagedDesktop,
    StandaloneCli,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedRuntime {
    pub executable: PathBuf,
    pub source: RuntimeSource,
}

/// Environment-backed resolver. Fields are injectable so discovery can be
/// tested without relying on the machine running the tests.
#[derive(Clone, Debug, Default)]
pub struct DesktopRuntimeResolver {
    local_app_data: Option<PathBuf>,
    program_files: Vec<PathBuf>,
    current_exe: Option<PathBuf>,
    path_entries: Vec<PathBuf>,
}

impl DesktopRuntimeResolver {
    pub fn from_environment() -> Self {
        let local_app_data = env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let mut program_files = Vec::new();
        for name in ["ProgramFiles", "ProgramW6432"] {
            if let Some(path) = env::var_os(name).map(PathBuf::from) {
                if !program_files.contains(&path) {
                    program_files.push(path);
                }
            }
        }

        Self {
            local_app_data,
            program_files,
            current_exe: env::current_exe().ok(),
            path_entries: env::var_os("PATH")
                .map(|value| env::split_paths(&value).collect())
                .unwrap_or_default(),
        }
    }

    #[cfg(test)]
    pub fn resolve(&self) -> Option<ResolvedRuntime> {
        self.resolve_candidates().into_iter().next()
    }

    /// Returns every usable-looking channel in preference order. Callers can
    /// continue to the standalone CLI if a stale Desktop runtime fails its
    /// app-server handshake after an update.
    pub fn resolve_candidates(&self) -> Vec<ResolvedRuntime> {
        let mut candidates = Vec::new();
        for candidate in [
            self.resolve_desktop_user_runtime(),
            self.resolve_desktop_install(),
            self.resolve_packaged_desktop(),
            self.resolve_path_cli(),
        ]
        .into_iter()
        .flatten()
        {
            if !candidates
                .iter()
                .any(|existing: &ResolvedRuntime| existing.executable == candidate.executable)
            {
                candidates.push(candidate);
            }
        }
        candidates
    }

    fn resolve_desktop_user_runtime(&self) -> Option<ResolvedRuntime> {
        let local = self.local_app_data.as_ref()?;
        // Versions below this directory are deliberately discovered rather
        // than embedded in the application.
        let root = local.join("OpenAI").join("Codex").join("bin");
        best_runtime_in(&[root], 4).map(|executable| ResolvedRuntime {
            executable,
            source: RuntimeSource::DesktopUserRuntime,
        })
    }

    fn resolve_desktop_install(&self) -> Option<ResolvedRuntime> {
        let mut roots = Vec::new();
        if let Some(local) = &self.local_app_data {
            roots.extend([
                local.join("Programs").join("OpenAI").join("Codex"),
                local.join("Programs").join("OpenAI").join("ChatGPT"),
                local.join("OpenAI").join("ChatGPT"),
            ]);
        }
        for base in &self.program_files {
            roots.extend([
                base.join("OpenAI").join("Codex"),
                base.join("OpenAI").join("ChatGPT"),
                base.join("ChatGPT"),
            ]);
        }

        // A portable/Desktop build can keep its runtime next to the current
        // executable or under a resources/bin descendant.
        if let Some(install_dir) = self.current_exe.as_deref().and_then(Path::parent) {
            roots.push(install_dir.to_path_buf());
            roots.push(install_dir.join("resources"));
            roots.push(install_dir.join("bin"));
            if let Some(parent) = install_dir.parent() {
                roots.push(parent.join("resources"));
                roots.push(parent.join("bin"));
            }
        }

        best_runtime_in(&roots, 6).map(|executable| ResolvedRuntime {
            executable,
            source: RuntimeSource::DesktopInstall,
        })
    }

    fn resolve_packaged_desktop(&self) -> Option<ResolvedRuntime> {
        let mut package_roots = Vec::new();

        // Package family names and versioned install directory names are
        // enumerated dynamically. No WindowsApps version path is embedded.
        if let Some(local) = &self.local_app_data {
            collect_matching_children(&local.join("Packages"), &mut package_roots);
        }
        for base in &self.program_files {
            collect_matching_children(&base.join("WindowsApps"), &mut package_roots);
        }

        best_runtime_in(&package_roots, 7).map(|executable| ResolvedRuntime {
            executable,
            source: RuntimeSource::PackagedDesktop,
        })
    }

    fn resolve_path_cli(&self) -> Option<ResolvedRuntime> {
        for directory in &self.path_entries {
            for name in runtime_file_names() {
                let candidate = directory.join(name);
                if candidate.is_file() {
                    return Some(ResolvedRuntime {
                        executable: candidate,
                        source: RuntimeSource::StandaloneCli,
                    });
                }
            }
        }
        None
    }
}

fn runtime_file_names() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &["codex.exe", "codex.cmd"]
    }
    #[cfg(not(windows))]
    {
        &["codex"]
    }
}

fn collect_matching_children(parent: &Path, output: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if (name.contains("openai") || name.contains("chatgpt") || name.contains("codex"))
            && entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false)
        {
            output.push(entry.path());
        }
    }
}

fn best_runtime_in(roots: &[PathBuf], max_depth: usize) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    let mut visited = HashSet::new();
    for root in roots {
        collect_runtimes(root, 0, max_depth, &mut visited, &mut candidates);
    }
    candidates.into_iter().max_by(|left, right| {
        candidate_rank(left)
            .cmp(&candidate_rank(right))
            .then_with(|| left.cmp(right))
    })
}

fn collect_runtimes(
    directory: &Path,
    depth: usize,
    max_depth: usize,
    visited: &mut HashSet<PathBuf>,
    output: &mut Vec<PathBuf>,
) {
    if depth > max_depth || !directory.is_dir() {
        return;
    }
    let identity = directory
        .canonicalize()
        .unwrap_or_else(|_| directory.to_path_buf());
    if !visited.insert(identity) {
        return;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_file() && is_runtime_file(&path) {
            output.push(path);
        } else if kind.is_dir() {
            collect_runtimes(&path, depth + 1, max_depth, visited, output);
        }
    }
}

fn is_runtime_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    runtime_file_names()
        .iter()
        .any(|expected| name.eq_ignore_ascii_case(expected))
}

fn candidate_rank(path: &Path) -> (Vec<u64>, SystemTime) {
    let version = path
        .components()
        .flat_map(|part| {
            part.as_os_str()
                .to_string_lossy()
                .split(|character: char| !character.is_ascii_digit())
                .filter(|part| !part.is_empty())
                .filter_map(|part| part.parse::<u64>().ok())
                .collect::<Vec<_>>()
        })
        .collect();
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    (version, modified)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let path =
                env::temp_dir().join(format!("desktop-runtime-resolver-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn executable(&self, relative: &str) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::File::create(&path).unwrap().write_all(b"test").unwrap();
            path
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn resolver(local: &Path, path_entries: Vec<PathBuf>) -> DesktopRuntimeResolver {
        DesktopRuntimeResolver {
            local_app_data: Some(local.to_path_buf()),
            program_files: Vec::new(),
            current_exe: None,
            path_entries,
        }
    }

    /// Runtime file name for the host platform. The resolver only matches
    /// `codex.exe`/`codex.cmd` on Windows and `codex` elsewhere, so the
    /// fixtures must not hard-code a single platform's name (CI runs on
    /// Windows while local runs are typically macOS/Linux).
    fn runtime_name() -> &'static str {
        runtime_file_names()[0]
    }

    #[test]
    fn desktop_user_runtime_wins_over_path_cli() {
        let tree = TempTree::new();
        let desktop = tree.executable(&format!("OpenAI/Codex/bin/1.2.3/{}", runtime_name()));
        tree.executable(&format!("cli/{}", runtime_name()));

        let found = resolver(&tree.0, vec![tree.0.join("cli")])
            .resolve()
            .unwrap();
        assert_eq!(found.executable, desktop);
        assert_eq!(found.source, RuntimeSource::DesktopUserRuntime);
    }

    #[test]
    fn desktop_and_cli_are_both_returned_for_runtime_fallback() {
        let tree = TempTree::new();
        let desktop = tree.executable(&format!("OpenAI/Codex/bin/2.0.0/{}", runtime_name()));
        let cli = tree.executable(&format!("cli/{}", runtime_name()));
        let found = resolver(&tree.0, vec![tree.0.join("cli")]).resolve_candidates();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].executable, desktop);
        assert_eq!(found[0].source, RuntimeSource::DesktopUserRuntime);
        assert_eq!(found[1].executable, cli);
        assert_eq!(found[1].source, RuntimeSource::StandaloneCli);
    }

    #[test]
    fn newest_dynamic_desktop_version_is_selected() {
        let tree = TempTree::new();
        tree.executable(&format!("OpenAI/Codex/bin/1.9.0/{}", runtime_name()));
        let newest = tree.executable(&format!("OpenAI/Codex/bin/1.10.0/{}", runtime_name()));

        let found = resolver(&tree.0, Vec::new()).resolve().unwrap();
        assert_eq!(found.executable, newest);
    }

    #[test]
    fn path_cli_is_only_a_fallback() {
        let tree = TempTree::new();
        let cli = tree.executable(&format!("cli/{}", runtime_name()));

        let found = resolver(&tree.0, vec![tree.0.join("cli")])
            .resolve()
            .unwrap();
        assert_eq!(found.executable, cli);
        assert_eq!(found.source, RuntimeSource::StandaloneCli);
    }

    #[test]
    fn runtime_next_to_desktop_resources_is_discovered() {
        let tree = TempTree::new();
        let desktop_exe = tree.executable("Desktop/ChatGPT.exe");
        let runtime = tree.executable(&format!("Desktop/resources/runtime/{}", runtime_name()));
        let found = DesktopRuntimeResolver {
            local_app_data: None,
            program_files: Vec::new(),
            current_exe: Some(desktop_exe),
            path_entries: Vec::new(),
        }
        .resolve()
        .unwrap();

        assert_eq!(found.executable, runtime);
        assert_eq!(found.source, RuntimeSource::DesktopInstall);
    }

    #[test]
    fn unrelated_package_directory_is_ignored() {
        let tree = TempTree::new();
        tree.executable("Packages/Unrelated.App_123/resources/codex.exe");

        assert!(resolver(&tree.0, Vec::new()).resolve().is_none());
    }
}
