use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use crate::audio::{AudioCapture, CpalAudioCapture};
use crate::client::{RealSagaClient, SagaClient};
use crate::config::MyrConfig;
use crate::dsl;
use crate::hyprland::{HyprlandExecutor, RealHyprlandExecutor};
use crate::notify::{DesktopNotifier, Notifier};
use crate::tunnel::{SshTunnel, TunnelConfig};

pub struct Daemon {
    #[allow(dead_code)]
    config: MyrConfig,
    client: Box<dyn SagaClient>,
    executor: Box<dyn HyprlandExecutor>,
    audio: Box<dyn AudioCapture>,
    notifier: Box<dyn Notifier>,
}

#[derive(Debug, PartialEq, Eq)]
enum Message {
    VoiceStart,
    VoiceStop,
    VoiceToggle,
    Text(String),
    Ping,
}

pub fn start(config: MyrConfig) -> anyhow::Result<()> {
    let tunnel_config = TunnelConfig {
        saga_host: config.saga_host.clone(),
        saga_voice_ip: config.saga_voice_ip.clone(),
        saga_voice_port: config.saga_voice_port.clone(),
        local_port: config.myr_local_port.clone(),
    };
    let _tunnel = SshTunnel::establish(tunnel_config)?;
    tracing::info!("SSH tunnel established");

    let executor = RealHyprlandExecutor::new()?;
    tracing::info!("Hyprland detected");

    let client = RealSagaClient::new(&config)?;
    let audio = CpalAudioCapture::new();
    let notifier = DesktopNotifier;

    let mut daemon = Daemon::new(
        config,
        Box::new(client),
        Box::new(executor),
        Box::new(audio),
        Box::new(notifier),
    );

    daemon.run()
}

impl Daemon {
    pub fn new(
        config: MyrConfig,
        client: Box<dyn SagaClient>,
        executor: Box<dyn HyprlandExecutor>,
        audio: Box<dyn AudioCapture>,
        notifier: Box<dyn Notifier>,
    ) -> Self {
        Self {
            config,
            client,
            executor,
            audio,
            notifier,
        }
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        let sock_path = socket_path();

        if sock_path.exists() {
            std::fs::remove_file(&sock_path)?;
            tracing::info!("Removed stale socket at {:?}", sock_path);
        }

        let listener = UnixListener::bind(&sock_path)?;
        tracing::info!("Listening on {:?}", sock_path);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    if let Err(e) = self.handle_connection(stream) {
                        tracing::error!("Error handling connection: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Error accepting connection: {}", e);
                }
            }
        }

        Ok(())
    }

    fn handle_connection(&mut self, stream: UnixStream) -> anyhow::Result<()> {
        let reader = BufReader::new(&stream);
        let mut writer = &stream;

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim().to_string();

            if trimmed.is_empty() {
                continue;
            }

            let response = match parse_message(&trimmed) {
                Some(msg) => self.handle_message(msg),
                None => format!("ERR:unknown command: {}", trimmed),
            };

            writeln!(writer, "{}", response)?;
        }

        Ok(())
    }

    fn handle_message(&mut self, msg: Message) -> String {
        match msg {
            Message::Ping => "PONG".to_string(),

            Message::VoiceStart => match self.audio.start() {
                Ok(()) => {
                    let _ = self.notifier.notify("Myr", "Listening...");
                    "OK:recording".to_string()
                }
                Err(e) => format!("ERR:{}", e),
            },

            Message::VoiceStop => match self.process_voice() {
                Ok(resp) => resp,
                Err(e) => {
                    let _ = self.notifier.error("Myr", &e.to_string());
                    format!("ERR:{}", e)
                }
            },

            Message::VoiceToggle => {
                if self.audio.is_recording() {
                    match self.process_voice() {
                        Ok(resp) => resp,
                        Err(e) => {
                            let _ = self.notifier.error("Myr", &e.to_string());
                            format!("ERR:{}", e)
                        }
                    }
                } else {
                    match self.audio.start() {
                        Ok(()) => {
                            let _ = self.notifier.notify("Myr", "Listening...");
                            "OK:recording".to_string()
                        }
                        Err(e) => format!("ERR:{}", e),
                    }
                }
            }

            Message::Text(ref text) => match self.process_text(text) {
                Ok(resp) => resp,
                Err(e) => {
                    let _ = self.notifier.error("Myr", &e.to_string());
                    format!("ERR:{}", e)
                }
            },
        }
    }

    fn process_voice(&mut self) -> anyhow::Result<String> {
        let wav_bytes = match self.audio.stop() {
            Ok(bytes) => bytes,
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("silent") || msg.contains("No audio") {
                    let _ = self.notifier.notify("Myr", "No speech detected");
                    return Ok("OK:stopped".to_string());
                }
                return Err(e);
            }
        };

        let windows = self.executor.list_windows()?;
        let context = windows.join("\n");

        let response = match self.client.send_audio(&wav_bytes, &context) {
            Ok(r) => r,
            Err(e) => {
                let _ = self.notifier.error("Myr", "Cannot reach Saga server");
                return Err(e);
            }
        };

        self.process_response(&response)
    }

    fn process_text(&mut self, text: &str) -> anyhow::Result<String> {
        let windows = self.executor.list_windows()?;
        let context = windows.join("\n");

        let response = match self.client.send_text(text, &context) {
            Ok(r) => r,
            Err(e) => {
                let _ = self.notifier.error("Myr", "Cannot reach Saga server");
                return Err(e);
            }
        };

        self.process_response(&response)
    }

    fn process_response(&self, response: &str) -> anyhow::Result<String> {
        let commands = match dsl::parse(response) {
            Ok(cmds) => cmds,
            Err(e) => {
                let _ = self
                    .notifier
                    .error("Myr", &format!("Could not understand command: {}", e));
                return Ok(format!("ERR:parse error: {}", e));
            }
        };

        if commands.is_empty() {
            let _ = self.notifier.notify("Myr", "Not a window command");
            return Ok("OK:stopped".to_string());
        }

        let results = execute_commands(self.executor.as_ref(), &commands);

        let mut successes = Vec::new();
        let mut errors = Vec::new();

        for (i, result) in results.iter().enumerate() {
            match result {
                Ok(()) => successes.push(format!("{:?}", commands[i].verb)),
                Err(e) => errors.push(format!("{:?}: {}", commands[i].verb, e)),
            }
        }

        if !successes.is_empty() {
            let summary = successes.join(", ");
            let _ = self
                .notifier
                .notify("Myr", &format!("Executed: {}", summary));
        }

        if !errors.is_empty() {
            for err in &errors {
                let _ = self.notifier.error("Myr", err);
            }
        }

        if successes.is_empty() && !errors.is_empty() {
            Ok("ERR:all commands failed".to_string())
        } else {
            Ok("OK".to_string())
        }
    }
}

pub fn socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(runtime_dir).join("myr.sock")
}

fn parse_message(line: &str) -> Option<Message> {
    let trimmed = line.trim();
    match trimmed {
        "VOICE_START" => Some(Message::VoiceStart),
        "VOICE_STOP" => Some(Message::VoiceStop),
        "VOICE_TOGGLE" => Some(Message::VoiceToggle),
        "PING" => Some(Message::Ping),
        s if s.starts_with("TEXT:") => {
            let text = s.strip_prefix("TEXT:").unwrap();
            if text.is_empty() {
                None
            } else {
                Some(Message::Text(text.to_string()))
            }
        }
        _ => None,
    }
}

fn execute_commands(
    executor: &dyn HyprlandExecutor,
    commands: &[dsl::Command],
) -> Vec<anyhow::Result<()>> {
    commands.iter().map(|cmd| executor.execute(cmd)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::MockAudioCapture;
    use crate::client::MockSagaClient;
    use crate::dsl::{Command, Selector, Verb};
    use crate::hyprland::MockHyprlandExecutor;
    use crate::notify::MockNotifier;

    fn make_daemon(
        client: MockSagaClient,
        executor: MockHyprlandExecutor,
        audio: MockAudioCapture,
        notifier: MockNotifier,
    ) -> Daemon {
        Daemon::new(
            MyrConfig::from_env(),
            Box::new(client),
            Box::new(executor),
            Box::new(audio),
            Box::new(notifier),
        )
    }

    #[test]
    fn test_parse_message_all_variants() {
        assert_eq!(parse_message("VOICE_START"), Some(Message::VoiceStart));
        assert_eq!(parse_message("VOICE_STOP"), Some(Message::VoiceStop));
        assert_eq!(parse_message("VOICE_TOGGLE"), Some(Message::VoiceToggle));
        assert_eq!(parse_message("PING"), Some(Message::Ping));
        assert_eq!(
            parse_message("TEXT:focus firefox"),
            Some(Message::Text("focus firefox".to_string()))
        );
        assert_eq!(parse_message("TEXT:"), None);
        assert_eq!(parse_message("UNKNOWN"), None);
        assert_eq!(parse_message(""), None);
    }

    #[test]
    fn test_parse_message_with_whitespace() {
        assert_eq!(parse_message("  PING  "), Some(Message::Ping));
        assert_eq!(
            parse_message("  TEXT:hello world  "),
            Some(Message::Text("hello world".to_string()))
        );
    }

    #[test]
    fn test_ping_response() {
        let client = MockSagaClient::new();
        let executor = MockHyprlandExecutor::new();
        let audio = MockAudioCapture::new();
        let notifier = MockNotifier::new();

        let mut daemon = make_daemon(client, executor, audio, notifier);
        let resp = daemon.handle_message(Message::Ping);
        assert_eq!(resp, "PONG");
    }

    #[test]
    fn test_voice_flow_happy_path() {
        let mut client = MockSagaClient::new();
        let mut executor = MockHyprlandExecutor::new();
        let mut audio = MockAudioCapture::new();
        let mut notifier = MockNotifier::new();

        audio
            .expect_stop()
            .times(1)
            .returning(|| Ok(vec![1, 2, 3, 4]));

        executor
            .expect_list_windows()
            .times(1)
            .returning(|| Ok(vec!["Firefox".to_string(), "Terminal".to_string()]));

        client
            .expect_send_audio()
            .with(
                mockall::predicate::eq(vec![1, 2, 3, 4]),
                mockall::predicate::eq("Firefox\nTerminal".to_string()),
            )
            .times(1)
            .returning(|_, _| Ok("FOCUS title:Firefox".to_string()));

        executor.expect_execute().times(1).returning(|_| Ok(()));

        notifier.expect_notify().times(1).returning(|_, _| Ok(()));

        let mut daemon = make_daemon(client, executor, audio, notifier);
        let resp = daemon.process_voice().unwrap();
        assert_eq!(resp, "OK");
    }

    #[test]
    fn test_voice_flow_silence() {
        let client = MockSagaClient::new();
        let executor = MockHyprlandExecutor::new();
        let mut audio = MockAudioCapture::new();
        let mut notifier = MockNotifier::new();

        audio
            .expect_stop()
            .times(1)
            .returning(|| Err(anyhow::anyhow!("Audio is silent (RMS amplitude: 0.001)")));

        notifier
            .expect_notify()
            .withf(|_, body| body == "No speech detected")
            .times(1)
            .returning(|_, _| Ok(()));

        let mut daemon = make_daemon(client, executor, audio, notifier);
        let resp = daemon.process_voice().unwrap();
        assert_eq!(resp, "OK:stopped");
    }

    #[test]
    fn test_voice_flow_no_audio_samples() {
        let client = MockSagaClient::new();
        let executor = MockHyprlandExecutor::new();
        let mut audio = MockAudioCapture::new();
        let mut notifier = MockNotifier::new();

        audio
            .expect_stop()
            .times(1)
            .returning(|| Err(anyhow::anyhow!("No audio samples captured")));

        notifier
            .expect_notify()
            .withf(|_, body| body == "No speech detected")
            .times(1)
            .returning(|_, _| Ok(()));

        let mut daemon = make_daemon(client, executor, audio, notifier);
        let resp = daemon.process_voice().unwrap();
        assert_eq!(resp, "OK:stopped");
    }

    #[test]
    fn test_voice_flow_none_response() {
        let mut client = MockSagaClient::new();
        let mut executor = MockHyprlandExecutor::new();
        let mut audio = MockAudioCapture::new();
        let mut notifier = MockNotifier::new();

        audio.expect_stop().times(1).returning(|| Ok(vec![1, 2, 3]));

        executor
            .expect_list_windows()
            .times(1)
            .returning(|| Ok(vec![]));

        client
            .expect_send_audio()
            .times(1)
            .returning(|_, _| Ok("NONE".to_string()));

        notifier
            .expect_notify()
            .withf(|_, body| body == "Not a window command")
            .times(1)
            .returning(|_, _| Ok(()));

        let mut daemon = make_daemon(client, executor, audio, notifier);
        let resp = daemon.process_voice().unwrap();
        assert_eq!(resp, "OK:stopped");
    }

    #[test]
    fn test_voice_flow_invalid_dsl() {
        let mut client = MockSagaClient::new();
        let mut executor = MockHyprlandExecutor::new();
        let mut audio = MockAudioCapture::new();
        let mut notifier = MockNotifier::new();

        audio.expect_stop().times(1).returning(|| Ok(vec![1, 2, 3]));

        executor
            .expect_list_windows()
            .times(1)
            .returning(|| Ok(vec![]));

        client
            .expect_send_audio()
            .times(1)
            .returning(|_, _| Ok("I don't understand that command".to_string()));

        notifier
            .expect_error()
            .withf(|_, body| body.contains("Could not understand command"))
            .times(1)
            .returning(|_, _| Ok(()));

        let mut daemon = make_daemon(client, executor, audio, notifier);
        let resp = daemon.process_voice().unwrap();
        assert!(resp.starts_with("ERR:parse error"));
    }

    #[test]
    fn test_voice_flow_server_error() {
        let mut client = MockSagaClient::new();
        let mut executor = MockHyprlandExecutor::new();
        let mut audio = MockAudioCapture::new();
        let mut notifier = MockNotifier::new();

        audio.expect_stop().times(1).returning(|| Ok(vec![1, 2, 3]));

        executor
            .expect_list_windows()
            .times(1)
            .returning(|| Ok(vec![]));

        client
            .expect_send_audio()
            .times(1)
            .returning(|_, _| Err(anyhow::anyhow!("Connection timeout")));

        notifier
            .expect_error()
            .withf(|_, body| body == "Cannot reach Saga server")
            .times(1)
            .returning(|_, _| Ok(()));

        let mut daemon = make_daemon(client, executor, audio, notifier);
        let resp = daemon.process_voice();
        assert!(resp.is_err());
        assert!(resp.unwrap_err().to_string().contains("Connection timeout"));
    }

    #[test]
    fn test_text_flow_happy_path() {
        let mut client = MockSagaClient::new();
        let mut executor = MockHyprlandExecutor::new();
        let audio = MockAudioCapture::new();
        let mut notifier = MockNotifier::new();

        executor
            .expect_list_windows()
            .times(1)
            .returning(|| Ok(vec!["Firefox".to_string()]));

        client
            .expect_send_text()
            .with(
                mockall::predicate::eq("focus firefox"),
                mockall::predicate::eq("Firefox".to_string()),
            )
            .times(1)
            .returning(|_, _| Ok("FOCUS title:Firefox".to_string()));

        executor.expect_execute().times(1).returning(|_| Ok(()));

        notifier
            .expect_notify()
            .withf(|_, body| body.contains("Executed"))
            .times(1)
            .returning(|_, _| Ok(()));

        let mut daemon = make_daemon(client, executor, audio, notifier);
        let resp = daemon.process_text("focus firefox").unwrap();
        assert_eq!(resp, "OK");
    }

    #[test]
    fn test_text_flow_executor_partial_failure() {
        let mut client = MockSagaClient::new();
        let mut executor = MockHyprlandExecutor::new();
        let audio = MockAudioCapture::new();
        let mut notifier = MockNotifier::new();

        executor
            .expect_list_windows()
            .times(1)
            .returning(|| Ok(vec!["Firefox".to_string()]));

        client
            .expect_send_text()
            .times(1)
            .returning(|_, _| Ok("FOCUS title:Firefox\nCLOSE title:Missing".to_string()));

        executor
            .expect_execute()
            .times(2)
            .returning(|cmd| match &cmd.selector {
                Selector::Title(t) if t == "Firefox" => Ok(()),
                _ => Err(anyhow::anyhow!("Window not found: Missing")),
            });

        notifier
            .expect_notify()
            .withf(|_, body| body.contains("Executed") && body.contains("Focus"))
            .times(1)
            .returning(|_, _| Ok(()));

        notifier
            .expect_error()
            .withf(|_, body| body.contains("Close") && body.contains("Window not found"))
            .times(1)
            .returning(|_, _| Ok(()));

        let mut daemon = make_daemon(client, executor, audio, notifier);
        let resp = daemon
            .process_text("focus firefox and close missing")
            .unwrap();
        assert_eq!(resp, "OK");
    }

    #[test]
    fn test_voice_toggle_starts_when_not_recording() {
        let client = MockSagaClient::new();
        let executor = MockHyprlandExecutor::new();
        let mut audio = MockAudioCapture::new();
        let mut notifier = MockNotifier::new();

        audio.expect_is_recording().times(1).returning(|| false);
        audio.expect_start().times(1).returning(|| Ok(()));

        notifier
            .expect_notify()
            .withf(|_, body| body == "Listening...")
            .times(1)
            .returning(|_, _| Ok(()));

        let mut daemon = make_daemon(client, executor, audio, notifier);
        let resp = daemon.handle_message(Message::VoiceToggle);
        assert_eq!(resp, "OK:recording");
    }

    #[test]
    fn test_voice_toggle_stops_when_recording() {
        let mut client = MockSagaClient::new();
        let mut executor = MockHyprlandExecutor::new();
        let mut audio = MockAudioCapture::new();
        let mut notifier = MockNotifier::new();

        audio.expect_is_recording().times(1).returning(|| true);
        audio.expect_stop().times(1).returning(|| Ok(vec![1, 2, 3]));

        executor
            .expect_list_windows()
            .times(1)
            .returning(|| Ok(vec![]));

        client
            .expect_send_audio()
            .times(1)
            .returning(|_, _| Ok("FOCUS title:Firefox".to_string()));

        executor.expect_execute().times(1).returning(|_| Ok(()));

        notifier.expect_notify().times(1).returning(|_, _| Ok(()));

        let mut daemon = make_daemon(client, executor, audio, notifier);
        let resp = daemon.handle_message(Message::VoiceToggle);
        assert_eq!(resp, "OK");
    }

    #[test]
    fn test_execute_commands_all_succeed() {
        let mut executor = MockHyprlandExecutor::new();
        executor.expect_execute().times(2).returning(|_| Ok(()));

        let commands = vec![
            Command {
                verb: Verb::Focus,
                selector: Selector::Title("Firefox".to_string()),
                args: vec![],
            },
            Command {
                verb: Verb::Close,
                selector: Selector::Class("Terminal".to_string()),
                args: vec![],
            },
        ];

        let results = execute_commands(&executor, &commands);
        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
    }

    #[test]
    fn test_execute_commands_partial_failure() {
        let mut executor = MockHyprlandExecutor::new();
        executor
            .expect_execute()
            .times(2)
            .returning(|cmd| match cmd.verb {
                Verb::Focus => Ok(()),
                Verb::Close => Err(anyhow::anyhow!("Window not found")),
                _ => Ok(()),
            });

        let commands = vec![
            Command {
                verb: Verb::Focus,
                selector: Selector::Title("Firefox".to_string()),
                args: vec![],
            },
            Command {
                verb: Verb::Close,
                selector: Selector::Title("Ghost".to_string()),
                args: vec![],
            },
        ];

        let results = execute_commands(&executor, &commands);
        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_err());
    }

    #[test]
    fn test_socket_path_ends_with_myr_sock() {
        let path = socket_path();
        assert!(path.ends_with("myr.sock"));
    }
}
