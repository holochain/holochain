use once_cell::sync::Lazy;

static PIPED: Lazy<std::sync::Mutex<bool>> = Lazy::new(|| std::sync::Mutex::new(false));

pub(crate) fn set_piped(piped: bool) {
    *PIPED.lock().unwrap() = piped;
}

fn get_piped() -> bool {
    *PIPED.lock().unwrap()
}

static PASSPHRASE: Lazy<Result<sodoken::BufRead, String>> = Lazy::new(|| {
    if get_piped() {
        read_piped_passphrase().map_err(|e| e.to_string())
    } else {
        read_interactive_passphrase("# passphrase> ").map_err(|e| e.to_string())
    }
});

pub(crate) fn get_passphrase() -> anyhow::Result<sodoken::BufRead> {
    PASSPHRASE
        .clone()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e).into())
}

fn vec_to_locked(mut pass_tmp: Vec<u8>) -> anyhow::Result<sodoken::BufRead> {
    match sodoken::BufWrite::new_mem_locked(pass_tmp.len()) {
        Err(e) => {
            pass_tmp.fill(0);
            Err(e.into())
        }
        Ok(p) => {
            {
                let mut lock = p.write_lock();
                lock.copy_from_slice(&pass_tmp);
                pass_tmp.fill(0);
            }
            Ok(p.to_read())
        }
    }
}

fn read_interactive_passphrase(prompt: &str) -> anyhow::Result<sodoken::BufRead> {
    let prompt = prompt.to_owned();
    let pass_tmp = rpassword::read_password_from_tty(Some(&prompt))?;
    vec_to_locked(pass_tmp.into_bytes())
}

fn read_piped_passphrase() -> anyhow::Result<sodoken::BufRead> {
    use std::io::Read;

    let stdin = std::io::stdin();
    let mut stdin = stdin.lock();
    let passphrase = <sodoken::BufWriteSized<512>>::new_mem_locked()?;
    let mut next_char = 0;
    loop {
        let mut lock = passphrase.write_lock();
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
            Err(e) => return Err(e.into()),
        };
        if done {
            if next_char == 0 {
                return Ok(sodoken::BufWrite::new_no_lock(0).to_read());
            }
            if lock[next_char - 1] == 13 {
                next_char -= 1;
            }
            let out = sodoken::BufWrite::new_mem_locked(next_char)?;
            {
                let mut out_lock = out.write_lock();
                out_lock.copy_from_slice(&lock[..next_char]);
            }
            return Ok(out.to_read());
        }
    }
}
