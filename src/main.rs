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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Daemon => {
            tracing::info!("daemon mode stub");
        }
        Commands::Do { text } => {
            tracing::info!(text = %text, "do command stub");
        }
        Commands::VoiceToggle => {
            tracing::info!("voice-toggle stub");
        }
    }

    Ok(())
}
