pub struct Commit(String);

impl AsRef<std::ffi::OsStr> for Commit {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.0.as_ref()
    }
}

impl From<String> for Commit {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// bench a specific commit by running the nix command for it
/// using a nix command guarantees that we are inside nix
/// requiring a nix shell allows us to make a _lot_ of assumptions
/// about how this command will behave like dependencies and environment vars
pub fn commit(commit: Commit) {
    match std::env::var("GITHUB_TOKEN") {
        Ok(token) => {
            match std::process::Command::new("hc-bench-github")
                .arg(commit)
                .arg(token)
                .spawn()
            {
                Ok(mut child) => match child.wait() {
                    Ok(_) => {}
                    Err(e) => eprintln!("bench error: {}", e),
                },
                Err(e) => eprintln!("command error: {}", e),
            }
        }
        Err(e) => eprintln!("token error: {}", e),
    }
}
