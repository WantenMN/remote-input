use std::path::Path;
use std::time::SystemTime;

fn run(cmd: &str, args: &[&str], dir: &str) {
    let mut command = if cfg!(target_os = "windows") {
        // On Windows, resolve .cmd/.ps1 shims (e.g. from nvm4w) via cmd.exe.
        let mut c = std::process::Command::new("cmd");
        c.arg("/c").arg(cmd);
        c.args(args);
        c
    } else {
        let mut c = std::process::Command::new(cmd);
        c.args(args);
        c
    };
    match command.current_dir(dir).status() {
        Ok(s) if s.success() => {}
        Ok(s) => panic!("{cmd} exited with {s}"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            panic!(
                "`{cmd}` not found. Install it with:\n  \
                 npm install -g pnpm\n  \
                 or: curl -fsSL https://get.pnpm.io/install.sh | sh -"
            );
        }
        Err(e) => panic!("failed to run `{cmd}`: {e}"),
    }
}

fn main() {
    // Re-run the build script when frontend sources or outputs change.
    println!("cargo:rerun-if-changed=web/src");
    println!("cargo:rerun-if-changed=web/index.html");
    println!("cargo:rerun-if-changed=web/package.json");
    println!("cargo:rerun-if-changed=web/pnpm-lock.yaml");
    println!("cargo:rerun-if-changed=web/vite.config.ts");
    println!("cargo:rerun-if-changed=web/tsconfig.json");
    println!("cargo:rerun-if-changed=web/node_modules/.modules.yaml");
    println!("cargo:rerun-if-changed=web-dist");

    // Install dependencies if node_modules is missing.
    if !Path::new("web/node_modules").exists() {
        run("pnpm", &["install"], "web");
    }

    // Skip the frontend build if the output already exists and is newer than
    // all sources. This avoids running the build when only Rust code changes.
    let web_src = Path::new("web/src");
    let dist = Path::new("web-dist/index.html");
    if dist.exists() {
        let dist_mtime = dist.metadata().unwrap().modified().unwrap();
        if web_src.exists() && dir_is_older(web_src, dist_mtime) {
            return;
        }
    }

    // Run the frontend build.
    run("pnpm", &["run", "build"], "web");
}

/// Returns `true` if every file in `dir` was last modified before `threshold`.
fn dir_is_older(dir: &Path, threshold: SystemTime) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return true;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            if !dir_is_older(&entry.path(), threshold) {
                return false;
            }
        } else if meta.modified().unwrap_or(SystemTime::UNIX_EPOCH) > threshold {
            return false;
        }
    }
    true
}
