#![allow(dead_code)]

/// The location of all our sql scrips
const SQL_DIR: &str = "./src/sql";

/// An env var that will trigger a SQL check.
const CHK_SQL_FMT: Option<&str> = option_env!("CHK_SQL_FMT");

/// An env var that will trigger a SQL format.
const FIX_SQL_FMT: Option<&str> = option_env!("FIX_SQL_FMT");

fn chk_sql_fmt() -> bool {
    if let Some(csf) = CHK_SQL_FMT {
        !csf.is_empty()
    } else {
        false
    }
}

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
        .unwrap_or_else(|e| panic!("Path doesn't exist: {path:?}. Error: {e}"))
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
        uppercase: Some(true),
        lines_between_queries: 2,
        ignore_case_convert: Some(vec!["Action", "lock", "type"]),
    };

    let fmt_sql = sqlformat::format(src_sql, &sqlformat::QueryParams::None, &opt);

    let fmt_sql = fmt_sql.trim();

    if fix_sql_fmt() {
        if src_sql != fmt_sql {
            std::fs::write(path, format!("{fmt_sql}\n")).unwrap();
            println!(
                "cargo:warning=FIX_SQL_FMT--fixing: {}",
                path.to_string_lossy()
            );
        }
    } else if chk_sql_fmt() {
        panic_on_diff(path, src_sql, fmt_sql);
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=CHK_SQL_FMT");
    println!("cargo:rerun-if-env-changed=FIX_SQL_FMT");
    let all_sql = find_sql(std::path::Path::new(SQL_DIR));
    for sql in all_sql {
        println!("cargo:rerun-if-changed={}", sql.to_string_lossy());
        check_fmt(&sql);
    }
}
