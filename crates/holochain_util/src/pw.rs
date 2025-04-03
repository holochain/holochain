//! Commandline passphrase capture utilities.

use once_cell::sync::Lazy;
use std::io::Result;
use std::sync::{Arc, Mutex};

static PIPED: Lazy<std::sync::Mutex<bool>> = Lazy::new(|| std::sync::Mutex::new(false));

/// Set the "piped" flag. If the user would prefer to send the passphrase
/// over stdin (rather than tty capture). This must be set before the first
/// call to [pw_get] or the passphrase will already be captured.
pub fn pw_set_piped(piped: bool) {
    *PIPED.lock().unwrap() = piped;
}

fn get_piped() -> bool {
    *PIPED.lock().unwrap()
}

static PASSPHRASE: Lazy<std::result::Result<Arc<Mutex<sodoken::LockedArray>>, String>> =
    Lazy::new(|| {
        if get_piped() {
            read_piped_passphrase().map_err(|e| e.to_string())
        } else {
            read_interactive_passphrase("# passphrase> ").map_err(|e| e.to_string())
        }
    });

/// Capture a passphrase from the user. Either captures from tty, or
/// reads stdin if [pw_set_piped] was called with `true`.
pub fn pw_get() -> Result<Arc<Mutex<sodoken::LockedArray>>> {
    PASSPHRASE
        .clone()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

fn read_interactive_passphrase(prompt: &str) -> Result<Arc<Mutex<sodoken::LockedArray>>> {
    let prompt = prompt.to_owned();
    let pass_tmp = rpassword::prompt_password(prompt)?;
    Ok(Arc::new(Mutex::new(pass_tmp.into_bytes().into())))
}

fn read_piped_passphrase() -> Result<Arc<Mutex<sodoken::LockedArray>>> {
    use std::io::Read;

    let stdin = std::io::stdin();
    let mut stdin = stdin.lock();
    let mut passphrase = sodoken::LockedArray::new(512)?;
    let mut next_char = 0;
    loop {
        let mut lock = passphrase.lock();
        let done = match stdin.read_exact(&mut lock[next_char..next_char + 1]) {
            Ok(_) => {
                if lock[next_char] == 10 {
                    true
                } else {
                    next_char += 1;
                    false
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => true,
            Err(e) => return Err(e),
        };
        if done {
            if next_char == 0 {
                return Ok(Arc::new(Mutex::new(sodoken::LockedArray::new(0)?)));
            }
            if lock[next_char - 1] == 13 {
                next_char -= 1;
            }
            let mut out = sodoken::LockedArray::new(next_char)?;
            {
                let mut out_lock = out.lock();
                out_lock.copy_from_slice(&lock[..next_char]);
            }
            return Ok(Arc::new(Mutex::new(out)));
        }
    }
}
