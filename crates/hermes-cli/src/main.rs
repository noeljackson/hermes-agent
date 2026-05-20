use std::env;
use std::path::PathBuf;
use std::process;

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args == ["--help"] || args == ["help"] {
        print!("{}", hermes_cli::TOP_LEVEL_HELP);
        return;
    }

    if args == ["--version"] || args == ["version"] {
        print!(
            "Hermes Agent v0.0.0-rust-parity\nProject: rust-rewrite\nPython: unavailable\nOpenAI SDK: unavailable\n"
        );
        return;
    }

    let hermes_home = env::var_os("HERMES_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".hermes")))
        .unwrap_or_else(|| PathBuf::from(".hermes"));

    let argv_strings = std::iter::once("hermes".to_string())
        .chain(args)
        .collect::<Vec<_>>();
    let argv = argv_strings.iter().map(String::as_str).collect::<Vec<_>>();

    match hermes_cli::run_safe_command_in_home(&argv, &hermes_home) {
        Ok(result) => {
            print!("{}", result.stdout);
            eprint!("{}", result.stderr);
            process::exit(result.exit_code);
        }
        Err(err) => {
            eprintln!("hermes: {err}");
            process::exit(1);
        }
    }
}
