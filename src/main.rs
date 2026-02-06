use clap::Parser;
use reqwest::multipart;
use std::io::Write;

mod audio;
mod hyprland;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// URL of the Voice API server
    #[arg(long, default_value = "http://localhost:8765")]
    server: String,

    /// Dry run: print commands without executing them
    #[arg(long)]
    dry_run: bool,
}

#[derive(serde::Deserialize, Debug)]
struct VoiceResponse {
    #[allow(dead_code)]
    raw: String,
    text: String,
    #[allow(dead_code)]
    saved_id: Option<String>,
    commands: Option<Vec<String>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("Myr Client - Connected to {}", args.server);
    println!("Ready.");

    loop {
        print!("\nPress Enter to start recording (or Ctrl+C to exit)...");
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        let audio_data = match audio::record_audio() {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to record audio: {}", e);
                continue;
            }
        };

        println!("Recorded {} bytes. Capturing layout...", audio_data.len());

        let layout = match hyprland::get_clients() {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Warning: Failed to get layout: {}. Commands may not be generated.", e);
                String::new()
            }
        };

        println!("Sending request to {}/voice...", args.server);

        let client = reqwest::Client::new();
        let part = multipart::Part::bytes(audio_data)
            .file_name("recording.wav")
            .mime_str("audio/wav")?;

        let mut form = multipart::Form::new()
            .part("audio", part);

        if !layout.is_empty() {
            form = form.text("layout", layout);
        }

        let res = client.post(format!("{}/voice", args.server))
            .multipart(form)
            .send()
            .await;

        match res {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<VoiceResponse>().await {
                        Ok(data) => {
                            println!("Transcribed: {}", data.text);
                            if let Some(commands) = data.commands {
                                if commands.is_empty() {
                                    println!("No commands generated.");
                                } else {
                                    println!("Generated Commands:");
                                    for cmd in &commands {
                                        println!("  > {}", cmd);
                                    }

                                    if !args.dry_run {
                                        println!("Executing...");
                                        for cmd in commands {
                                            if let Err(e) = hyprland::dispatch(&cmd) {
                                                eprintln!("Error executing '{}': {}", cmd, e);
                                            }
                                        }
                                        println!("Done.");
                                    } else {
                                        println!("Dry run mode. Skipping execution.");
                                    }
                                }
                            } else {
                                println!("No commands returned.");
                            }
                        }
                        Err(e) => eprintln!("Failed to parse response: {}", e),
                    }
                } else {
                    eprintln!("Server returned error: {}", response.status());
                    if let Ok(text) = response.text().await {
                        eprintln!("Body: {}", text);
                    }
                }
            }
            Err(e) => eprintln!("Request failed: {}", e),
        }
    }
}
