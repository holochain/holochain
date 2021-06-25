const SQL_DIR: &str = "./src/sql";
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
    for e in std::fs::read_dir(path).unwrap() {
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
        println!("cargo:rerun-if-changed={}", path.to_string_lossy());
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

    let opt = sqlformat::FormatOptions {
        indent: sqlformat::Indent::Spaces(2),
        uppercase: true,
        lines_between_queries: 1,
    };

    let fmt_sql = sqlformat::format(&src_sql, &sqlformat::QueryParams::None, opt);
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

fn main() {
    println!("cargo:rerun-if-env-changed=FIX_SQL_FMT");
    for sql in find_sql(&std::path::Path::new(SQL_DIR)) {
        check_fmt(&sql);
    }
}
