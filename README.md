# ON AIR

A macOS utility that turns an LED board on while Microsoft Teams is using the
microphone, then turns it off after the microphone has been idle for three
seconds.

## Configure

Set the LED board endpoint in `.env`:

```sh
ONAIR_URL=http://your-led-board/power
ONAIR_HOME_SSID=My Home Wi-Fi
```

Set `ONAIR_HOME_SSID` to the exact (case-sensitive) name of your home Wi-Fi
network.

Recent macOS versions hide the Wi-Fi name from command-line tools. Run this
once to allow the bridge to read it (the setting persists across reboots):

```sh
sudo ipconfig setverbose 1
```

Verify it with `ipconfig getsummary en0 | grep ' SSID'`; it should show your
network name instead of `<redacted>`.

Before polling Teams or sending LED requests, the bridge checks that the Mac
is connected to the configured Wi-Fi network. Away from home, offline, or on
Ethernet without Wi-Fi, HTTP requests are paused. The bridge automatically
resumes and resynchronizes the LED when home is detected again. Network status
is logged only on changes; identical network check errors are logged once until
recovery. Matching on the network name is not an authentication mechanism.

The Teams process names, polling interval, and off delay can be configured at
the top of `src/main.rs`.

## Run

Requires Rust and macOS:

```sh
cargo run --release
```

The bridge sends `POST` requests to the configured LED URL with a JSON body of
`{"state": true}` while Teams is using the microphone and `{"state": false}`
when it switches off.

## Start automatically on macOS

Choose one of the options below; do not enable both, or two copies will run.
Both options start the bridge when you log in, not before login.

First, build the standalone binary from the project directory:

```sh
cargo build --release
```

Keep the project directory (including `.env` and `target/release/onair`) in a
stable location. The working directory must be the project directory so the
bridge can find `.env`.

### Option 1: LaunchAgent (recommended)

A LaunchAgent runs in the background and restarts the bridge if it exits.

1. Create the configuration and log directories:

   ```sh
   mkdir -p "$HOME/Library/LaunchAgents" "$HOME/Library/Logs/onair"
   ```

2. Save the following as `~/Library/LaunchAgents/local.onair.plist`.
   Replace `/Users/YOUR_USERNAME/path/to/onair` with the absolute project path
   and `/Users/YOUR_USERNAME` with your home directory. Do not use `~` or
   `$HOME` inside the plist; launchd does not expand them. If a path contains
   `&`, write it as `&amp;` in the XML.

   ```xml
   <?xml version="1.0" encoding="UTF-8"?>
   <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
   <plist version="1.0">
   <dict>
       <key>Label</key>
       <string>local.onair</string>
       <key>ProgramArguments</key>
       <array>
           <string>/Users/YOUR_USERNAME/path/to/onair/target/release/onair</string>
       </array>
       <key>WorkingDirectory</key>
       <string>/Users/YOUR_USERNAME/path/to/onair</string>
       <key>RunAtLoad</key>
       <true/>
       <key>KeepAlive</key>
       <true/>
       <key>StandardOutPath</key>
       <string>/Users/YOUR_USERNAME/Library/Logs/onair/stdout.log</string>
       <key>StandardErrorPath</key>
       <string>/Users/YOUR_USERNAME/Library/Logs/onair/stderr.log</string>
   </dict>
   </plist>
   ```

3. Validate the plist, enable it, and start it now (no `sudo` needed):

   ```sh
   plutil -lint "$HOME/Library/LaunchAgents/local.onair.plist"
   launchctl enable "gui/$(id -u)/local.onair"
   launchctl bootstrap "gui/$(id -u)" "$HOME/Library/LaunchAgents/local.onair.plist"
   ```

Check its status and logs:

```sh
launchctl print "gui/$(id -u)/local.onair"
tail -n 50 "$HOME/Library/Logs/onair/stdout.log" "$HOME/Library/Logs/onair/stderr.log"
```

After rebuilding the binary or changing `.env`, restart the bridge:

```sh
launchctl kickstart -k "gui/$(id -u)/local.onair"
```

To stop it and disable automatic startup:

```sh
launchctl bootout "gui/$(id -u)/local.onair"
launchctl disable "gui/$(id -u)/local.onair"
```

To re-enable it later, repeat step 3.

### Option 2: Automator application as a Login Item

This is a GUI-based alternative without automatic restart.

1. Open **Automator**, create a new **Application**, and add a **Run Shell
   Script** action.
2. Enter the following, replacing the path with the absolute project path:

   ```sh
   cd "/Users/YOUR_USERNAME/path/to/onair" || exit 1
   mkdir -p "$HOME/Library/Logs/onair" || exit 1
   exec ./target/release/onair >> "$HOME/Library/Logs/onair/stdout.log" \
       2>> "$HOME/Library/Logs/onair/stderr.log"
   ```

3. Save it as **ON AIR.app** in your Applications folder.
4. Open **System Settings > General > Login Items & Extensions** (called
   **Login Items** on some macOS versions), and add the application under
   **Open at Login**.
5. Open the application once to start it immediately. Avoid opening it again
   while the bridge is already running.

To disable startup, remove the application from **Open at Login**. To stop a
running copy, open **Activity Monitor**, find the `onair` process, and quit it.
