use std::{
    error::Error,
    io,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use chrono::Local;
use reqwest::blocking::Client;
use serde_json::json;

mod network;

const TEAMS_PROCESSES: &[&str] = &["MSTeams", "Microsoft Teams"];
const POLL_INTERVAL: Duration = Duration::from_secs(1);
const OFF_DELAY: Duration = Duration::from_secs(3);

#[cfg(target_os = "macos")]
mod core_audio {
    use std::{ffi::c_void, io};

    #[repr(C)]
    struct AudioObjectPropertyAddress {
        selector: u32,
        scope: u32,
        element: u32,
    }

    #[link(name = "CoreAudio", kind = "framework")]
    unsafe extern "C" {
        fn AudioObjectGetPropertyData(
            object_id: u32,
            address: *const AudioObjectPropertyAddress,
            qualifier_data_size: u32,
            qualifier_data: *const c_void,
            data_size: *mut u32,
            data: *mut c_void,
        ) -> i32;
    }

    const SYSTEM_OBJECT: u32 = 1;
    const DEFAULT_INPUT_DEVICE: u32 = fourcc(b"dIn ");
    const DEVICE_IS_RUNNING: u32 = fourcc(b"gone");
    const GLOBAL_SCOPE: u32 = fourcc(b"glob");

    const fn fourcc(value: &[u8; 4]) -> u32 {
        u32::from_be_bytes(*value)
    }

    fn property_u32(object_id: u32, selector: u32) -> io::Result<u32> {
        let address = AudioObjectPropertyAddress {
            selector,
            scope: GLOBAL_SCOPE,
            element: 0,
        };
        let mut value = 0_u32;
        let mut data_size = std::mem::size_of::<u32>() as u32;
        // CoreAudio writes one u32 to `value` when the property query succeeds.
        let status = unsafe {
            AudioObjectGetPropertyData(
                object_id,
                &address,
                0,
                std::ptr::null(),
                &mut data_size,
                (&mut value as *mut u32).cast(),
            )
        };

        if status != 0 {
            return Err(io::Error::other(format!("CoreAudio error {status}")));
        }

        Ok(value)
    }

    pub fn microphone_in_use() -> io::Result<bool> {
        let device = property_u32(SYSTEM_OBJECT, DEFAULT_INPUT_DEVICE)?;
        if device == 0 {
            return Ok(false);
        }

        Ok(property_u32(device, DEVICE_IS_RUNNING)? != 0)
    }
}

#[cfg(not(target_os = "macos"))]
mod core_audio {
    use std::io;

    pub fn microphone_in_use() -> io::Result<bool> {
        Err(io::Error::other(
            "the ON AIR bridge requires macOS CoreAudio",
        ))
    }
}

fn teams_running() -> io::Result<bool> {
    for process in TEAMS_PROCESSES {
        let status = Command::new("pgrep")
            .args(["-x", process])
            .stdout(Stdio::null())
            .status()?;
        if status.success() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn set_led(client: &Client, led_url: &str, on: bool) -> Result<(), Box<dyn Error>> {
    client
        .post(led_url)
        .json(&json!({ "state": on }))
        .send()?
        .error_for_status()?;
    Ok(())
}

fn timestamp() -> String {
    Local::now().format("%H:%M:%S").to_string()
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv()?;
    let led_url = std::env::var("ONAIR_URL").map_err(|_| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "ONAIR_URL is not set in the .env file",
        )
    })?;
    let home_router_mac = std::env::var("ONAIR_HOME_ROUTER_MAC")
        .map_err(|_| io::Error::other("ONAIR_HOME_ROUTER_MAC is not set in the .env file"))?;
    let home_router_mac = network::parse_mac(&home_router_mac)
        .map_err(|error| io::Error::other(format!("invalid ONAIR_HOME_ROUTER_MAC: {error}")))?;
    let client = Client::builder().timeout(Duration::from_secs(5)).build()?;

    let mut led_state = None;
    let mut idle_since: Option<Instant> = None;
    let mut home_state = None;
    let mut network_error = None;

    loop {
        let at_home = match network::at_home(&home_router_mac) {
            Ok(at_home) => {
                network_error = None;
                at_home
            }
            Err(error) => {
                let message = error.to_string();
                if network_error.as_ref() != Some(&message) {
                    eprintln!("network check error: {message}");
                    network_error = Some(message);
                }
                false
            }
        };
        if home_state != Some(at_home) {
            println!(
                "{} {}",
                timestamp(),
                if at_home {
                    "home network detected"
                } else {
                    "not on home network; LED requests paused"
                }
            );
            home_state = Some(at_home);
        }
        if !at_home {
            led_state = None;
            idle_since = None;
            thread::sleep(POLL_INTERVAL);
            continue;
        }

        let poll_result = (|| -> Result<(), Box<dyn Error>> {
            let busy = teams_running()? && core_audio::microphone_in_use()?;
            if busy {
                idle_since = None;
                if led_state != Some(true) {
                    match set_led(&client, &led_url, true) {
                        Ok(()) => {
                            led_state = Some(true);
                            println!("{} ON AIR", timestamp());
                        }
                        Err(error) => eprintln!("LED error: {error}"),
                    }
                }
            } else {
                let since = idle_since.get_or_insert_with(Instant::now);
                if led_state != Some(false) && since.elapsed() >= OFF_DELAY {
                    match set_led(&client, &led_url, false) {
                        Ok(()) => {
                            led_state = Some(false);
                            println!("{} off", timestamp());
                        }
                        Err(error) => eprintln!("LED error: {error}"),
                    }
                }
            }
            Ok(())
        })();

        if let Err(error) = poll_result {
            eprintln!("error: {error}");
        }

        thread::sleep(POLL_INTERVAL);
    }
}
