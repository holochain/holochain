use std::path::PathBuf;

/// The location of all our sql scrips
const SQL_DIR: &str = "./src/sql";

/// An env var that will trigger a SQL format.
const FIX_SQL_FMT: Option<&str> = option_env!("FIX_SQL_FMT");

/// Git ref against which to check for schema changes
const GIT_BASE_REF: &str = "origin/develop";

fn fix_sql_fmt() -> bool {
    if let Some(fsf) = FIX_SQL_FMT {
        !fsf.is_empty()
    } else {
        false
    }
}

fn find_sql(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(path)
        .unwrap_or_else(|e| panic!("Path doesn't exist: {:?}. Error: {}", path, e))
    {
        let e = e.unwrap();
        let path = e.path();
        let t = e.file_type().unwrap();
        if t.is_dir() {
            out.append(&mut find_sql(&path));
            continue;
        }
        if !t.is_file() {
            continue;
        }
        if path.extension() != Some(std::ffi::OsStr::new("sql")) {
            continue;
        }
        out.push(path);
    }
    out
}

const FIX_MSG: &str = "-- `FIX_SQL_FMT=1 cargo build` to fix --";
fn panic_on_diff(path: &std::path::Path, s1: &str, s2: &str) {
    let s1 = s1.split('\n').collect::<Vec<_>>();
    let s2 = s2.split('\n').collect::<Vec<_>>();
    pretty_assertions::assert_eq!((s1, path, FIX_MSG), (s2, path, FIX_MSG));
}

fn check_fmt(path: &std::path::Path) {
    let src_sql = std::fs::read_to_string(path).unwrap();
    let src_sql = src_sql.trim();

    if src_sql.contains("no-sql-format") {
        return;
    }

    let opt = sqlformat::FormatOptions {
        indent: sqlformat::Indent::Spaces(2),
        uppercase: false,
        lines_between_queries: 1,
    };

    let fmt_sql = sqlformat::format(src_sql, &sqlformat::QueryParams::None, opt);

    let fmt_sql = fmt_sql.trim();

    if fix_sql_fmt() {
        if src_sql != fmt_sql {
            std::fs::write(path, format!("{}\n", fmt_sql)).unwrap();
            println!(
                "cargo:warning=FIX_SQL_FMT--fixing: {}",
                path.to_string_lossy()
            );
        }
    } else {
        panic_on_diff(path, src_sql, fmt_sql);
    }
}

/// Ensure that schema migration files have no changes since `GIT_BASE_REF` (origin/develop).
/// Pure additions are excluded, which allows new files to be created, but once they are merged
/// as a PR, then no changes can be made.
/// This is a loose check and obviously can be easily circumvented, but it hopefully helps remind us
/// that we must never change migration files.
fn check_migrations() {
    use std::process::Command;
    let root = PathBuf::from(SQL_DIR);

    // If git is unavailable, skip this check
    if match Command::new("git").arg("status").output() {
        Ok(output) => !output.status.success(),
        Err(_) => true,
    } {
        eprintln!("git or .git not available, cannot check schema migration files.");
        return;
    }

    for dir in [
        root.join("cell/schema"),
        root.join("conductor/schema"),
        root.join("p2p_agent_store/schema"),
        root.join("p2p_metrics/schema"),
        root.join("wasm/schema"),
    ] {
        for path in find_sql(&dir) {
            let mut cmd = Command::new("git");
            match cmd
                .arg("diff")
                // Filter out files which were merely added
                .arg("--diff-filter=a")
                .arg(GIT_BASE_REF)
                .arg(path.clone())
                .output()
            {
                Ok(out) => {
                    if out.status.success() && out.stdout.is_empty() && out.stderr.is_empty() {
                        // no change. good.
                    } else {
                        panic!("Diff found in schema file:\n\n{}\nSchema and migration files cannot be modified. Instead, set up a new database migration in 'crates/holochain_sqlite/src/schema.rs'\n\n", String::from_utf8_lossy(&out.stdout))
                    }
                }
                Err(err) => panic!("Error while checking schema: {:?}, path = {:?}", err, path),
            }
        }
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=FIX_SQL_FMT");
    let all_sql = find_sql(std::path::Path::new(SQL_DIR));
    for sql in all_sql {
        println!("cargo:rerun-if-changed={}", sql.to_string_lossy());
        check_fmt(&sql);
    }
    check_migrations();
}
