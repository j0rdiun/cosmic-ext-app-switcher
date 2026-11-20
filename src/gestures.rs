//! `--gestures`: three-finger swipe up shows the workspace overview, swipe down hides it.
//!
//! cosmic-comp claims every swipe of three or more fingers and does nothing with three
//! (`3 => None, // TODO: 3 finger gestures` in its src/input/mod.rs), so Wayland clients
//! never receive them. We read the touchpad's evdev node directly instead, which needs
//! read access to /dev/input (the `input` group). The compositor keeps reading the same
//! node; evdev hands every reader its own copy of the events.

use anyhow::{bail, Result};
use std::ffi::c_ulong;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::process::Command;

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;
const SYN_REPORT: u16 = 0;
const SYN_DROPPED: u16 = 3;
const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
/// Set by the kernel while exactly three fingers are on the pad.
const BTN_TOOL_TRIPLETAP: u16 = 0x14e;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Swipe {
    Up,
    Down,
}

/// Turns one touchpad's event stream into at most one swipe per three-finger touch.
pub struct Detector {
    min_distance: i32,
    pos: (i32, i32),
    start: Option<(i32, i32)>,
    three_down: bool,
    fired: bool,
}

impl Detector {
    pub fn new(min_distance: i32) -> Self {
        Self { min_distance, pos: (0, 0), start: None, three_down: false, fired: false }
    }

    pub fn feed(&mut self, ty: u16, code: u16, value: i32) -> Option<Swipe> {
        match (ty, code) {
            (EV_ABS, ABS_X) => self.pos.0 = value,
            (EV_ABS, ABS_Y) => self.pos.1 = value,
            (EV_KEY, BTN_TOOL_TRIPLETAP) => {
                self.three_down = value != 0;
                self.start = None;
                self.fired = false;
            }
            // The kernel's buffer overflowed and we lost events: drop the touch in progress.
            (EV_SYN, SYN_DROPPED) => self.three_down = false,
            (EV_SYN, SYN_REPORT) if self.three_down && !self.fired => {
                // ABS_X/ABS_Y follow one finger, which is enough to tell the direction.
                let Some((x0, y0)) = self.start else {
                    self.start = Some(self.pos);
                    return None;
                };
                let (dx, dy) = (self.pos.0 - x0, self.pos.1 - y0);
                if dy.abs() >= self.min_distance && dy.abs() > 2 * dx.abs() {
                    self.fired = true;
                    // Touchpad y grows downwards, so fingers moving up give a negative dy.
                    return Some(if dy < 0 { Swipe::Up } else { Swipe::Down });
                }
            }
            _ => {}
        }
        None
    }
}

/// Bit `bit` of a sysfs capability bitmap: 64-bit hex words, most significant first.
fn has_bit(bitmap: &str, bit: usize) -> bool {
    bitmap
        .split_whitespace()
        .rev()
        .nth(bit / 64)
        .and_then(|w| u64::from_str_radix(w, 16).ok())
        .is_some_and(|w| (w >> (bit % 64)) & 1 == 1)
}

/// Every input device that can report three fingers, i.e. touchpads.
// ponytail: scanned once at startup, so a touchpad plugged in later is ignored until relogin.
fn touchpads() -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir("/sys/class/input") else { return Vec::new() };
    let mut found: Vec<PathBuf> = dir
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("event"))
        .filter(|e| {
            std::fs::read_to_string(e.path().join("device/capabilities/key"))
                .is_ok_and(|keys| has_bit(&keys, BTN_TOOL_TRIPLETAP as usize))
        })
        .map(|e| PathBuf::from("/dev/input").join(e.file_name()))
        .collect();
    found.sort();
    found
}

#[repr(C)]
#[derive(Default)]
struct AbsInfo {
    value: i32,
    minimum: i32,
    maximum: i32,
    fuzz: i32,
    flat: i32,
    resolution: i32,
}

unsafe extern "C" {
    fn ioctl(fd: i32, request: c_ulong, ...) -> i32;
}

/// The touchpad's height in device units, via EVIOCGABS(ABS_Y).
fn pad_height(pad: &File) -> std::io::Result<i32> {
    // _IOR('E', 0x40 + ABS_Y, struct input_absinfo)
    let request: c_ulong = (2 << 30)
        | ((size_of::<AbsInfo>() as c_ulong) << 16)
        | ((b'E' as c_ulong) << 8)
        | (0x40 + ABS_Y as c_ulong);
    let mut info = AbsInfo::default();
    // SAFETY: EVIOCGABS writes exactly one input_absinfo, which AbsInfo mirrors.
    if unsafe { ioctl(pad.as_raw_fd(), request, &mut info as *mut AbsInfo) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(info.maximum - info.minimum)
}

/// Calls one of the overview's own D-Bus methods (`Show`/`Hide`; spawning the binary
/// would only toggle, so swipe down could open it). False if nothing answered.
fn call_overview(method: &str) -> bool {
    let path = "/com/system76/CosmicWorkspaces";
    let iface = "com.system76.CosmicWorkspaces";
    Command::new("busctl")
        .args(["--user", "call", iface, path, iface, method])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn overview(swipe: Swipe) {
    if swipe == Swipe::Down {
        // Nothing to close if the overview isn't running.
        call_overview("Hide");
        return;
    }
    if call_overview("Show") {
        return;
    }
    // cosmic-workspaces panics now and then ("Surface does not support the adapter's queue
    // family", pop-os/cosmic-workspaces-epoch#340), and cosmic-session stops restarting it
    // after a few tries, which leaves Super+W dead too. Start it ourselves: a fresh
    // instance claims its bus name within ~200 ms and starts hidden, so Show it once it's up.
    log::warn!("workspace overview is not running, starting cosmic-workspaces");
    let started = Command::new("setsid")
        .args(["-f", "cosmic-workspaces"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if let Err(e) = started {
        log::error!("could not start cosmic-workspaces: {e}");
        return;
    }
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if call_overview("Show") {
            return;
        }
    }
    log::error!("cosmic-workspaces did not come up within 2 s");
}

fn watch(path: PathBuf, mut pad: File, swipe_distance: f32) -> Result<()> {
    let height = pad_height(&pad)?;
    let min_distance = ((height as f32 * swipe_distance) as i32).max(1);
    log::info!("{}: height {height}, swipe after {min_distance} units", path.display());
    let mut detector = Detector::new(min_distance);
    // struct input_event on 64-bit Linux: timeval (16 bytes), type u16, code u16, value i32.
    let mut ev = [0u8; 24];
    loop {
        pad.read_exact(&mut ev)?;
        let ty = u16::from_ne_bytes([ev[16], ev[17]]);
        let code = u16::from_ne_bytes([ev[18], ev[19]]);
        let value = i32::from_ne_bytes([ev[20], ev[21], ev[22], ev[23]]);
        if let Some(swipe) = detector.feed(ty, code, value) {
            overview(swipe);
        }
    }
}

pub fn run(swipe_distance: f32) -> Result<()> {
    if !(swipe_distance > 0.0 && swipe_distance <= 1.0) {
        bail!("--swipe-distance must be above 0 and at most 1, got {swipe_distance}");
    }
    let pads = touchpads();
    if pads.is_empty() {
        bail!("no touchpad found under /sys/class/input");
    }

    let mut threads = Vec::new();
    for path in &pads {
        match File::open(path) {
            Ok(pad) => {
                let path = path.clone();
                threads.push(std::thread::spawn(move || watch(path, pad, swipe_distance)));
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => bail!(
                "cannot read {}: add yourself to the input group with \
                 `sudo usermod -aG input $USER`, then log out and back in",
                path.display()
            ),
            Err(e) => log::error!("skipping {}: {e}", path.display()),
        }
    }
    for t in threads {
        if let Ok(Err(e)) = t.join() {
            log::error!("touchpad reader stopped: {e}");
        }
    }
    bail!("no touchpad left to read")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One evdev frame: the given (type, code, value) events followed by SYN_REPORT.
    fn frame(d: &mut Detector, events: &[(u16, u16, i32)]) -> Option<Swipe> {
        for &(ty, code, value) in events {
            assert_eq!(d.feed(ty, code, value), None);
        }
        d.feed(EV_SYN, SYN_REPORT, 0)
    }

    fn swipe(fingers_down: (u16, u16), moves: &[(i32, i32)]) -> Vec<Swipe> {
        let mut d = Detector::new(100);
        let mut out = Vec::new();
        let (x, y) = moves[0];
        out.extend(frame(&mut d, &[(EV_ABS, ABS_X, x), (EV_ABS, ABS_Y, y), (EV_KEY, fingers_down.0, fingers_down.1 as i32)]));
        for &(x, y) in &moves[1..] {
            out.extend(frame(&mut d, &[(EV_ABS, ABS_X, x), (EV_ABS, ABS_Y, y)]));
        }
        out.extend(frame(&mut d, &[(EV_KEY, fingers_down.0, 0)]));
        out
    }

    #[test]
    fn three_finger_vertical_swipes_fire_once() {
        let up = [(500, 800), (505, 700), (510, 650), (515, 400)];
        assert_eq!(swipe((BTN_TOOL_TRIPLETAP, 1), &up), vec![Swipe::Up]);
        let down = [(500, 200), (500, 350), (500, 600)];
        assert_eq!(swipe((BTN_TOOL_TRIPLETAP, 1), &down), vec![Swipe::Down]);
    }

    #[test]
    fn short_sideways_or_two_finger_moves_do_nothing() {
        assert!(swipe((BTN_TOOL_TRIPLETAP, 1), &[(500, 500), (500, 450)]).is_empty());
        assert!(swipe((BTN_TOOL_TRIPLETAP, 1), &[(500, 500), (300, 380)]).is_empty());
        const BTN_TOOL_DOUBLETAP: u16 = 0x14d;
        assert!(swipe((BTN_TOOL_DOUBLETAP, 1), &[(500, 800), (500, 400)]).is_empty());
    }

    #[test]
    fn capability_bitmap_matches_a_real_touchpad() {
        // /sys/class/input/event5/device/capabilities/key of a Synaptics SYNA802F pad.
        let synaptics = "e520 10000 0 0 0 0";
        assert!(has_bit(synaptics, BTN_TOOL_TRIPLETAP as usize));
        assert!(!has_bit("10000 0 0 0 0", BTN_TOOL_TRIPLETAP as usize));
        assert!(!has_bit("", BTN_TOOL_TRIPLETAP as usize));
    }
}
