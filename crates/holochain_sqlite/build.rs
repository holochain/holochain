use std::path::PathBuf;

/// The location of all our sql scrips
const SQL_DIR: &str = "./src/sql";

/// An env var that will trigger a SQL format.
const FIX_SQL_FMT: Option<&str> = option_env!("FIX_SQL_FMT");

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

fn _check_migrations() {
    let root = PathBuf::from(SQL_DIR);
    for dir in [
        root.join("cell/schema"),
        root.join("conductor/schema"),
        root.join("p2p_agent_store/schema"),
        root.join("p2p_metrics/schema"),
        root.join("wasm/schema"),
    ] {
        for _path in find_sql(&dir) {
            // TODO: ensure that each schema migration script not introduced "recently"
            // (for some value of "recently") has not changed. We don't ever
            // want these to change, we only want to add new ones.
            // Probably the best way to accomplish this is through a git commit hook or something.
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
}
