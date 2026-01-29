use clap::{Parser, Subcommand};
use colored::Colorize;
use rodio::Source;
use rodio::{Decoder, Sink};
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;
use tracing::info;

const SOCKET_PATH: &str = "/tmp/sound.sock";

#[derive(Debug, Clone)]
enum Actions {
    Play,
    Pause,
    Resume,
    Clear,
    Queue,
    Skip,
    Current,
}

impl Actions {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "play" => Some(Actions::Play),
            "pause" => Some(Actions::Pause),
            "resume" => Some(Actions::Resume),
            "clear" => Some(Actions::Clear),
            "queue" => Some(Actions::Queue),
            "skip" => Some(Actions::Skip),
            "current" => Some(Actions::Current),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
struct TrackInfo {
    name: String,
    duration: usize,
}
struct Player {
    _output_stream: rodio::OutputStream,
    sink: Sink,
    queue: Vec<TrackInfo>,
}

#[derive(Deserialize)]
struct Response {
    status: bool,
    message: String,
}

impl Player {
    pub fn new() -> Self {
        let stream_handle =
            rodio::OutputStreamBuilder::open_default_stream().expect("open default audio stream");
        let sink = rodio::Sink::connect_new(&stream_handle.mixer());
        Player {
            _output_stream: stream_handle,
            sink: sink,
            queue: Vec::new(),
        }
    }

    pub fn push_to_queue(
        &mut self,
        source: impl Source + Send + 'static,
        metadata: TrackInfo,
    ) -> Value {
        self.sink.append(source);
        self.queue.push(metadata.clone());
        return json!({
            "status": true,
            "message": format!("{} was successfully added to the queue", &metadata.name)
        });
    }

    pub fn clear_queue(&mut self) -> Value {
        self.sink.clear();
        self.queue.clear();
        return json!({
            "status": true,
            "message": "Queue was successfully cleared"
        });
    }

    pub fn pause(&mut self) -> Value {
        if self.sink.len() == 0 {
            return json!({
                "status": false,
                "message": "Nothing is being played to pause"
            });
        }

        if self.sink.is_paused() {
            return json!({
                "status": true,
                "message": "Already paused"
            });
        } else {
            self.sink.pause();
            return json!({
                "status": true,
                "message": "Paused successfully"
            });
        }
    }

    pub fn resume(&mut self) -> Value {
        if self.sink.len() == 0 {
            return json!({
                "status": false,
                "message": "Nothing to resume"
            });
        }

        if self.sink.is_paused() {
            self.sink.play();
            return json!({
                "status": true,
                "message": "Resumed successfully"
            });
        } else {
            return json!({
                "status": true,
                "message": "Already playing"
            });
        }
    }

    pub fn play(&mut self, source: impl Source + Send + 'static, metadata: TrackInfo) -> Value {
        if !self.sink.empty() && !self.sink.is_paused() {
            return json!({
                "status": false,
                "message": "Already playing"
            });
        } else {
            self.sink.clear();
            self.queue.clear();
            self.sink.append(source);
            self.sink.play();

            self.queue.push(metadata.clone());
            return json!({
                "status": true,
                "message": format!("Now playing {}", metadata.name)
            });
        }
    }

    pub fn skip(&mut self) -> Value {
        if self.queue.is_empty() && self.sink.len() == 0 {
            return json!({
                "status": false,
                "message": "Nothing to skip"
            });
        } else {
            if !self.queue.is_empty() {
                let skipped = self.queue.remove(0);
                self.sink.skip_one();

                return json!({
                    "status": true,
                    "message": format!("Skipped {}", skipped.name)
                });
            } else {
                return json!({
                    "status": false,
                    "message": "Queue is empty"
                });
            }
        }
    }

    pub fn current(&mut self) -> Value {
        if self.queue.is_empty() && self.sink.len() == 0 {
            return json!({
                "status": false,
                "message": "Nothing is being played"
            });
        } else {
            let current_track = self.queue.first().unwrap();

            return json!({
                "status": true,
                "message": format!("Currently playing {}", current_track.name),
                "track": current_track.name.clone(),
                "queue_length": self.queue.len()
            });
        }
    }
}

#[derive(Deserialize, Debug)]
struct Command {
    action: String,
    track: Option<String>,
}

#[derive(Parser)]
#[command(name = "socket_app")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Play { track: String },
    Pause,
    Resume,
    Daemon,
    Queue { track: String },
    Clear,
    Skip,
    Current,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Daemon => {
            let _ = run_daemon().await;
        }
        Commands::Play { track } => {
            let res = send_command("play", Some(track)).await;
            colored_print(res);
        }
        Commands::Pause => {
            let res = send_command("pause", None).await;
            colored_print(res);
        }
        Commands::Skip => {
            let res = send_command("skip", None).await;
            colored_print(res);
        }
        Commands::Queue { track } => {
            let res = send_command("queue", Some(track)).await;
            colored_print(res);
        }
        Commands::Clear => {
            let res = send_command("clear", None).await;
            colored_print(res);
        }
        Commands::Resume => {
            let res = send_command("resume", None).await;
            colored_print(res);
        }
        Commands::Current => {
            let res = send_command("current", None).await;
            colored_print(res);
        }
    }
}

async fn run_daemon() {
    println!("Initializing socket connection");

    if Path::new(SOCKET_PATH).exists() {
        let _ = std::fs::remove_file(SOCKET_PATH);
    }
    let listener = tokio::net::UnixListener::bind(SOCKET_PATH).unwrap();
    let mut player = Player::new();

    loop {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 1024];

        if let Ok(n) = stream.read(&mut buf).await {
            if n == 0 {
                return;
            }

            let msg = &buf[..n];

            match serde_json::from_slice::<Command>(msg) {
                Ok(cmd) => {
                    let response = audio_controls(cmd, &mut player).await;
                    let response_str = response.to_string();
                    if let Err(e) = stream.write_all(response_str.as_bytes()).await {
                        eprintln!("Failed to send response: {}", e);
                    }
                }
                Err(e) => {
                    let error_response = json!({
                        "status": false,
                        "message": format!("Invalid JSON: {}", e)
                    });
                    let _ = stream
                        .write_all(error_response.to_string().as_bytes())
                        .await;
                }
            }
        }
    }
}

async fn audio_controls(cmd: Command, player: &mut Player) -> Value {
    // Parse action
    let action = match Actions::from_str(&cmd.action) {
        Some(a) => a,
        None => {
            return json!({
                "status": false,
                "message": format!("Invalid action: {}", cmd.action)
            });
        }
    };

    match action {
        Actions::Play => {
            let track = match cmd.track {
                Some(t) => t,
                None => {
                    return json!({
                        "status": false,
                        "message": "No track specified"
                    });
                }
            };

            let file = match File::open(&track) {
                Ok(f) => f,
                Err(e) => {
                    return json!({
                        "status": false,
                        "message": format!("Failed to open file: {}", e)
                    });
                }
            };

            let source = match Decoder::new(BufReader::new(file)) {
                Ok(s) => s,
                Err(e) => {
                    return json!({
                        "status": false,
                        "message": format!("Failed to decode audio: {}", e)
                    });
                }
            };

            let duration = source.total_duration().unwrap_or_default();
            let metadata = TrackInfo {
                name: track.clone(),
                duration: duration.as_secs() as usize,
            };
            player.play(source, metadata)
        }
        Actions::Pause => player.pause(),
        Actions::Clear => player.clear_queue(),
        Actions::Queue => {
            let track = match cmd.track {
                Some(t) => t,
                None => {
                    return json!({
                        "status": false,
                        "message": "No track specified"
                    });
                }
            };

            let file = match File::open(&track) {
                Ok(f) => f,
                Err(e) => {
                    return json!({
                        "status": false,
                        "message": format!("Failed to open file: {}", e)
                    });
                }
            };

            let source = match Decoder::new(BufReader::new(file)) {
                Ok(s) => s,
                Err(e) => {
                    return json!({
                        "status": false,
                        "message": format!("Failed to decode audio: {}", e)
                    });
                }
            };

            let duration = source.total_duration().unwrap_or_default();
            let metadata = TrackInfo {
                name: track.clone(),
                duration: duration.as_secs() as usize,
            };
            player.push_to_queue(source, metadata)
        }
        Actions::Skip => player.skip(),
        Actions::Resume => player.resume(),
        Actions::Current => player.current(),
    }
}

async fn send_command(action: &str, track: Option<String>) -> Value {
    let mut stream = match UnixStream::connect(SOCKET_PATH).await {
        Ok(stream) => stream,
        Err(e) => {
            let res = json!({"status":false,"message":format!("{} \nPlease make sure that daemon is running.",e)});
            return res;
        }
    };

    let _ = match Actions::from_str(action) {
        Some(a) => a,
        None => {
            return json!({
                "status": false,
                "message": format!("Invalid action: {}", action)
            });
        }
    };

    let cmd = json!({"action":action,"track":track});
    stream.write_all(cmd.to_string().as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.unwrap();

    let res_str = String::from_utf8_lossy(&buf).to_string();
    serde_json::from_str(&res_str).unwrap()
}

fn colored_print(res: Value) {
    let response: Response = serde_json::from_value(res.clone()).unwrap();
    if response.status == true {
        println!("{}", response.message.blue());
    } else {
        println!("{}", response.message.red())
    }
}
