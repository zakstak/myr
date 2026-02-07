use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use clap::{Parser, Subcommand};

/// myr — local agent companion for saga
#[derive(Parser)]
#[command(name = "myr", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the persistent daemon
    Daemon,
    /// Execute a natural-language command
    Do {
        /// The text command to execute
        text: String,
    },
    /// Toggle voice capture on/off
    VoiceToggle,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Daemon => {
            let config = myr::config::MyrConfig::from_env();
            myr::daemon::start(config)?;
        }
        Commands::Do { text } => {
            send_socket_command(&format!("TEXT:{}", text))?;
        }
        Commands::VoiceToggle => {
            send_socket_command("VOICE_TOGGLE")?;
        }
    }

    Ok(())
}

fn send_socket_command(message: &str) -> anyhow::Result<()> {
    let socket_path = myr::daemon::socket_path();

    let mut stream = match UnixStream::connect(&socket_path) {
        Ok(s) => s,
        Err(e)
            if e.kind() == std::io::ErrorKind::ConnectionRefused
                || e.kind() == std::io::ErrorKind::NotFound =>
        {
            eprintln!("Myr daemon not running. Start with: myr daemon");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to connect to daemon: {}", e);
            std::process::exit(1);
        }
    };

    writeln!(stream, "{}", message)?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;

    let response = response.trim();

    if response.starts_with("OK") {
        if response == "OK:recording" {
            println!("Recording...");
        } else if response == "OK:stopped" {
            println!("Processing stopped");
        } else {
            println!("Success");
        }
        Ok(())
    } else if let Some(err_msg) = response.strip_prefix("ERR:") {
        eprintln!("Error: {}", err_msg);
        std::process::exit(1);
    } else {
        eprintln!("Unexpected response: {}", response);
        std::process::exit(1);
    }
}
