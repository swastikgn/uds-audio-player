# udsaudio

A lightweight background audio player for Unix systems. It runs as a daemon and accepts commands via Unix Domain Sockets, allowing control through the included CLI or custom scripts.

## Features

- **Daemon Mode**: Handle audio playback in the background.
- **Queueing**: Add multiple tracks to a playback queue.
- **Controls**: Standard play, pause, resume, and skip functionality.
- **Socket API**: Simple JSON-based communication over `/tmp/sound.sock`.
- **CLI**: Intuitive command-line interface with colored output.

## Installation

Ensure you have Rust installed, then build the release binary:

```bash
cargo build --release
```

The executable will be located at `target/release/udsaudio`.

## Usage

### 1. Start the Daemon
The daemon must be running to process audio and listen for commands:

```bash
cargo run -- daemon
```

### 2. Control Commands
Run these commands in a separate terminal to control the active daemon.

- **Play Immediately**: Stops current playback, clears the queue, and starts the track.
  ```bash
  cargo run -- play path/to/track.wav
  ```

- **Add to Queue**: Appends a track to the end of the current queue.
  ```bash
  cargo run -- queue path/to/track.wav
  ```

- **Playback Control**:
  ```bash
  cargo run -- pause   # Pause playback
  cargo run -- resume  # Resume playback
  cargo run -- skip    # Skip the current track
  ```

- **Status and Maintenance**:
  ```bash
  cargo run -- current # Show active track and queue length
  cargo run -- clear   # Stop playback and empty the queue
  ```

## Technical Integration

### Socket Protocol
The engine listens at `/tmp/sound.sock`. You can control it by sending JSON packets:

```json
{
  "action": "play",
  "track": "path/to/file.wav"
}
```

Supported actions: `play`, `pause`, `resume`, `clear`, `queue`, `skip`, `current`.

### Example (Python)
```python
import json
import socket

with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
    client.connect("/tmp/sound.sock")
    cmd = {"action": "play", "track": "music.mp3"}
    client.sendall(json.dumps(cmd).encode("utf-8"))
    print(client.recv(4096).decode("utf-8"))
```

## Dependencies

- **rodio**: Audio playback and decoding.
- **tokio**: Asynchronous networking and runtime.
- **clap**: Command-line argument parsing.
- **serde**: JSON serialization and deserialization.
- **colored**: Terminal output styling.
