use crossterm::ExecutableCommand;
use futures::stream::{BoxStream, StreamExt};
use kitsune_p2p_proxy::tx2::*;
use kitsune_p2p_proxy::ProxyUrl;
use kitsune_p2p_transport_quic::tx2::*;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_pool_promote::*;
use kitsune_p2p_types::*;
use std::collections::HashSet;
use std::io::Write;

kitsune_p2p_types::write_codec_enum! {
    codec Wire {
        Null(0x00) {},
        Msg(0x01) {
            usr.0: String,
            msg.1: String,
        },
    }
}

#[derive(Debug)]
enum Evt {
    InCon(Tx2ConHnd<Wire>),
    Output(String),
    Key(char),
    Backspace,
    Enter,
    End,
}

fn spawn_evt() -> (tokio::sync::mpsc::Sender<Evt>, BoxStream<'static, Evt>) {
    use crossterm::event::{poll, read, Event, KeyCode::*, KeyModifiers};
    let (s_o, mut r_o) = tokio::sync::mpsc::channel(32);
    let (s, mut r) = tokio::sync::mpsc::channel(32);
    tokio::task::spawn_blocking(move || {
        loop {
            if poll(std::time::Duration::from_millis(500)).unwrap() {
                match read().unwrap() {
                    Event::Key(event) => {
                        // first catch ctrl-escapes
                        if event.modifiers.contains(KeyModifiers::CONTROL)
                            && (event.code == Char('c') || event.code == Char('d'))
                        {
                            if s.blocking_send(Evt::End).is_err() {
                                return;
                            }
                            continue;
                        }

                        let evt = match event.code {
                            Char(c) => Evt::Key(c),
                            Backspace => Evt::Backspace,
                            Enter => Evt::Enter,
                            _ => Evt::End,
                        };

                        if s.blocking_send(evt).is_err() {
                            return;
                        }
                    }
                    _ => (),
                }
            }
        }
    });
    let r_o =
        futures::stream::poll_fn(move |cx| -> std::task::Poll<Option<Evt>> { r_o.poll_recv(cx) })
            .boxed();
    let r = futures::stream::poll_fn(move |cx| -> std::task::Poll<Option<Evt>> { r.poll_recv(cx) })
        .boxed();
    (s_o, futures::stream::select_all(vec![r, r_o]).boxed())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let parse_args = || {
        let args = std::env::args().collect::<Vec<_>>();
        if args.len() != 3 {
            return Err(());
        }
        let username = args.get(1).unwrap().to_string();
        let proxy_url = ProxyUrl::from(args.get(2).unwrap());
        Ok((username, proxy_url))
    };

    let (username, proxy_url) = match parse_args() {
        Err(_) => {
            eprintln!("usage: cli-chat username proxy-url");
            std::process::exit(127);
        }
        Ok(r) => r,
    };

    println!("[cli-chat: start]");
    println!("[cli-chat: username: {}]", username);
    println!("[cli-chat: proxy_url: {}]", proxy_url);

    let conf = QuicConfig::default();
    let f = tx2_quic_adapter(conf).await.unwrap();
    let f = tx2_pool_promote(f, Default::default());
    let f = tx2_proxy(f, Default::default());
    let f = tx2_api::<Wire>(f, Default::default());

    let ep = f
        .bind(
            "kitsune-quic://0.0.0.0:0",
            KitsuneTimeout::from_millis(5000),
        )
        .await
        .unwrap();

    let ep_hnd = ep.handle().clone();

    let raw_addr = ep_hnd.local_addr().unwrap();
    println!("[cli-chat: local raw addr: {}]", raw_addr);

    let _ = ep_hnd
        .get_connection(
            proxy_url.as_str().to_string(),
            KitsuneTimeout::from_millis(1000 * 30),
        )
        .await
        .unwrap();

    let local_digest = ProxyUrl::from(raw_addr.as_str());
    let local_digest = local_digest.digest();
    let local_addr = ProxyUrl::new(proxy_url.as_base().as_str(), local_digest).unwrap();
    println!("[cli-chat: local proxy addr: {}]", local_addr);
    println!("\n--- local proxy addr - share this one ---");
    println!("{}", local_addr);
    println!("--- ----- ----- ---- - ----- ---- --- ---\n");
    println!("type '/help' for a list of commands.");

    crossterm::terminal::enable_raw_mode().unwrap();

    let (send_output, mut evt) = spawn_evt();

    let s_o_2 = send_output.clone();
    tokio::task::spawn(async move {
        let s_o_2 = &s_o_2;
        ep.for_each_concurrent(32, move |evt| async move {
            use Tx2EpEvent::*;
            match evt {
                IncomingRequest(Tx2EpIncomingRequest {
                    con, data, respond, ..
                }) => {
                    match data {
                        Wire::Msg(Msg { usr, msg }) => {
                            let _ = s_o_2
                                .send(Evt::Output(format!("{} says: {}", usr, msg)))
                                .await;
                        }
                        _ => (),
                    }
                    let _ = s_o_2.send(Evt::InCon(con)).await;
                    let _ = respond
                        .respond(Wire::null(), KitsuneTimeout::from_millis(1000 * 5))
                        .await;
                }
                Tick => (),
                evt => {
                    let _ = s_o_2
                        .send(Evt::Output(format!("[cli-chat evt: {:?}]", evt)))
                        .await;
                }
            }
        })
        .await;
    });

    let mut stdout = std::io::stdout();
    let mut line = String::new();
    let mut con_set = HashSet::new();

    // clear the current line restoring the current prompt
    macro_rules! rline {
        ($($t:tt)*) => {{
            use crossterm::cursor::MoveToColumn;
            use crossterm::terminal::{Clear, ClearType::*};
            stdout.execute(Clear(CurrentLine)).unwrap();
            stdout.execute(MoveToColumn(0)).unwrap();
            if line.len() > 60 {
                write!(
                    stdout,
                    "{}>... {}",
                    username,
                    line.chars().skip(line.len() - 60).collect::<String>()
                )
                .unwrap();
            } else {
                write!(stdout, "{}> {}", username, line).unwrap();
            }
            stdout.flush().unwrap();
        }};
    }

    // clear current line + print text, advancing the scroll - like println!
    macro_rules! pline {
        ($($t:tt)*) => {{
            use crossterm::terminal::{Clear, ClearType::*};
            use crossterm::cursor::{MoveToColumn};
            stdout.execute(Clear(CurrentLine)).unwrap();
            stdout.execute(MoveToColumn(0)).unwrap();
            write!(stdout, $($t)*).unwrap();
            write!(stdout, "\n").unwrap();
        }};
    }

    // replace current line with status text, without advancing scroll
    macro_rules! sline {
        ($($t:tt)*) => {{
            use crossterm::terminal::{Clear, ClearType::*};
            use crossterm::cursor::{MoveToColumn};
            stdout.execute(Clear(CurrentLine)).unwrap();
            stdout.execute(MoveToColumn(0)).unwrap();
            write!(stdout, $($t)*).unwrap();
            stdout.flush().unwrap();
        }};
    }

    rline!();
    while let Some(evt) = evt.next().await {
        match evt {
            Evt::InCon(c) => {
                con_set.insert(c);
            }
            Evt::Output(o) => pline!("{}", o),
            Evt::Key(evt) => line.push(evt),
            Evt::Backspace => {
                line.pop();
            }
            Evt::Enter => {
                if line.as_bytes().len() >= 2 && line.as_bytes()[0] as char == '/' {
                    pline!("{}", line);
                    match line.as_bytes()[1] as char {
                        'h' => {
                            pline!("");
                            pline!("[cli-chat commands]:");
                            pline!("/help - this help text");
                            pline!("/connect peer_url - connect to a remote peer");
                            pline!("/quit | /exit - exit cli-chat");
                            pline!("");
                        }
                        'e' | 'q' => break,
                        'c' => {
                            let (_, url) =
                                line.split_at(line.find(char::is_whitespace).unwrap() + 1);
                            let url = ProxyUrl::from(url);

                            let con = ep_hnd
                                .get_connection(
                                    url.to_string(),
                                    KitsuneTimeout::from_millis(1000 * 30),
                                )
                                .await
                                .unwrap();

                            con_set.insert(con);

                            pline!("connected to {}", url);
                        }
                        _ => pline!("unknown command: {}", line),
                    }
                    line.clear();
                } else {
                    let cons = con_set.drain().collect::<Vec<_>>();
                    for c in cons.into_iter() {
                        sline!("-- sending to con {:?} --", c.uniq());
                        match c
                            .request(
                                &Wire::msg(username.clone(), line.clone()),
                                KitsuneTimeout::from_millis(1000 * 5),
                            )
                            .await
                        {
                            Ok(_) => {
                                con_set.insert(c);
                            }
                            Err(e) => {
                                pline!("send error: {:?}", e);
                            }
                        }
                    }
                    send_output
                        .send(Evt::Output(format!("{} says: {}", username, line)))
                        .await
                        .unwrap();
                    line.clear();
                }
            }
            Evt::End => break,
        }
        rline!();
    }
    crossterm::terminal::disable_raw_mode().unwrap();
    println!("\n[cli-chat: done]");
    std::process::exit(0);
}
