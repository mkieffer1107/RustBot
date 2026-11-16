//! Terminal presentation only; all bot decisions use the shared Session.
use bot::support::engine::Session;
use pyrt::Value;
use std::io::{self, BufRead, IsTerminal, Write};
use std::time::Instant;

#[derive(Default)]
struct Options {
    prompt: Option<String>,
    interactive: bool,
    seed: Option<String>,
    verbose: bool,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Self::default();
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let (name, attached) = if arg.starts_with("-p") && arg.len() > 2 {
                ("-p", Some(arg[2..].strip_prefix('=').unwrap_or(&arg[2..])))
            } else if arg.starts_with("-s") && arg.len() > 2 {
                ("-s", Some(arg[2..].strip_prefix('=').unwrap_or(&arg[2..])))
            } else if arg.starts_with("--") && arg.contains('=') {
                let (name, value) = arg.split_once('=').unwrap();
                (name, Some(value))
            } else {
                (arg.as_str(), None)
            };
            match name {
                "-p" | "--prompt" => {
                    options.prompt = Some(match attached {
                        Some(value) => value.to_owned(),
                        None => args.next().ok_or("--prompt needs text")?.clone(),
                    });
                }
                "-s" | "--seed" => {
                    let seed = match attached {
                        Some(value) => value.to_owned(),
                        None => args.next().ok_or("--seed needs an integer")?.clone(),
                    };
                    seed.parse::<num_bigint::BigInt>()
                        .map_err(|e| e.to_string())?;
                    options.seed = Some(seed);
                }
                "-i" | "--interactive" if attached.is_none() => options.interactive = true,
                "-v" | "--verbose" if attached.is_none() => options.verbose = true,
                _ => return Err(format!("unknown option: {arg}; use --help")),
            }
        }
        Ok(options)
    }
}

fn python_whitespace(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | '\u{1c}'..='\u{20}' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

fn normalized(text: &str) -> String {
    let result = text
        .split(python_whitespace)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if result.is_empty() {
        "Hello!".into()
    } else {
        result
    }
}

fn answer(session: &mut Session, text: &str) -> (String, String, f64) {
    let start = Instant::now();
    let result = (|| -> pyrt::PyResult<_> {
        let decision = session.turn(text)?;
        let reply = pyrt::formatting::str(&decision.dict_get(&Value::from("reply"))?)?;
        let module = decision.dict_get(&Value::from("module"))?;
        Ok((
            normalized(&reply),
            module.as_str().unwrap_or("none").to_owned(),
        ))
    })();
    let (reply, module) = result.unwrap_or_else(|_| ("Hello!".into(), "none".into()));
    (reply, module, start.elapsed().as_secs_f64())
}

fn details(module: &str, seconds: f64) -> String {
    format!("[module: {module} | {:.0} ms]", seconds * 1000.0)
}

pub fn run(args: &[String]) -> Result<(), String> {
    let options = Options::parse(args)?;
    let seed = options
        .seed
        .as_deref()
        .unwrap_or("0")
        .parse::<num_bigint::BigInt>()
        .map_err(|e| e.to_string())?;
    let clock = std::env::var("PYBOT_CLOCK")
        .map(Ok)
        .unwrap_or_else(|_| system_clock())?;
    let mut session = Session::new(seed, &clock).map_err(|e| e.to_string())?;
    let interactive = (options.prompt.is_none() && io::stdin().is_terminal())
        || (options.prompt.is_some() && options.interactive);
    if interactive {
        return chat(&mut session, options);
    }
    if let Some(prompt) = options.prompt {
        let (reply, module, seconds) = answer(&mut session, &prompt);
        println!("{reply}");
        if options.verbose {
            eprintln!("{}", details(&module, seconds));
        }
    } else {
        for line in io::stdin().lock().lines() {
            let line = line.map_err(|e| e.to_string())?;
            let (reply, module, seconds) =
                answer(&mut session, line.trim_end_matches(['\r', '\n']));
            println!("{reply}");
            io::stdout().flush().map_err(|e| e.to_string())?;
            if options.verbose {
                eprintln!("{}", details(&module, seconds));
            }
        }
    }
    Ok(())
}

fn opening() {
    println!(
        "RustBot\n\nA chatbot written in Rust. No weights, no training, just rules.\nType a message and press Enter. /reset starts a new conversation, /exit quits.\n"
    );
}

fn chat(session: &mut Session, options: Options) -> Result<(), String> {
    opening();
    let mut first = options.prompt;
    loop {
        let text = if let Some(text) = first.take() {
            println!("user: {text}");
            text
        } else {
            print!("user: ");
            io::stdout().flush().map_err(|e| e.to_string())?;
            let mut line = String::new();
            if io::stdin()
                .read_line(&mut line)
                .map_err(|e| e.to_string())?
                == 0
            {
                println!();
                break;
            }
            line.trim_end_matches(['\r', '\n']).to_owned()
        };
        match text.trim_matches(python_whitespace).to_lowercase().as_str() {
            "/exit" | "/quit" | "/bye" => break,
            "/help" => {
                println!(
                    "Commands:\n  /reset   start a new conversation\n  /help    show this help\n  /exit    quit (Ctrl+D works too)\n"
                );
                continue;
            }
            "/reset" => {
                session.memory = Value::dict();
                if options.seed.is_some() {
                    session.runtime.random.reset_global();
                }
                if io::stdout().is_terminal() {
                    print!("\x1b[H\x1b[2J\x1b[3J");
                    opening();
                } else {
                    println!("New conversation.\n");
                }
                continue;
            }
            "" => continue,
            _ => {}
        }
        let (reply, module, seconds) = answer(session, &text);
        println!("rustbot: {reply}");
        if options.verbose {
            println!("{}", details(&module, seconds));
        }
        println!();
    }
    Ok(())
}

#[cfg(unix)]
fn system_clock() -> Result<String, String> {
    let seconds = unsafe { libc::time(std::ptr::null_mut()) };
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
    if unsafe { libc::localtime_r(&seconds, local.as_mut_ptr()) }.is_null() {
        return Err("could not read local clock".into());
    }
    let local = unsafe { local.assume_init() };
    Ok(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        local.tm_year + 1900,
        local.tm_mon + 1,
        local.tm_mday,
        local.tm_hour,
        local.tm_min,
        local.tm_sec
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_options_preserve_text_and_unbounded_seed() {
        let args = [
            "-p",
            " hello\nthere ",
            "-i",
            "-v",
            "-s",
            "-123456789012345678901234567890",
        ]
        .map(str::to_owned);
        let options = Options::parse(&args).unwrap();
        assert_eq!(options.prompt.as_deref(), Some(" hello\nthere "));
        assert_eq!(
            options.seed.as_deref(),
            Some("-123456789012345678901234567890")
        );
        assert!(options.interactive && options.verbose);
        for (args, prompt, seed) in [
            (
                vec!["--prompt=hello", "--seed=-7"],
                Some("hello"),
                Some("-7"),
            ),
            (vec!["-phello", "-s-7"], Some("hello"), Some("-7")),
            (vec!["--prompt="], Some(""), None),
            (vec!["-p=-x", "-s=-7"], Some("-x"), Some("-7")),
        ] {
            let parsed =
                Options::parse(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).unwrap();
            assert_eq!(parsed.prompt.as_deref(), prompt);
            assert_eq!(parsed.seed.as_deref(), seed);
        }
        for args in [vec!["-p"], vec!["-s"], vec!["-s", "1.2"], vec!["--unknown"]] {
            assert!(
                Options::parse(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err()
            );
        }
    }

    #[test]
    fn reply_normalization_includes_python_control_whitespace() {
        assert_eq!(normalized("\u{1c}hello\u{85}\u{a0}world\n"), "hello world");
        assert_eq!(normalized(" \t\u{1f}"), "Hello!");
        assert_eq!(normalized("a\u{200b}b"), "a\u{200b}b");
        assert_eq!("\u{1c}/exit\u{1f}".trim_matches(python_whitespace), "/exit");
    }

    #[test]
    fn system_clock_is_a_valid_runtime_clock() {
        pyrt::datetime::Clock::parse(&system_clock().unwrap()).unwrap();
    }
}

#[cfg(not(unix))]
fn system_clock() -> Result<String, String> {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let date = pyrt::datetime::PyDate::fromordinal(719163 + (seconds / 86400) as i64)
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "{}T{:02}:{:02}:{:02}",
        date.isoformat(),
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    ))
}
