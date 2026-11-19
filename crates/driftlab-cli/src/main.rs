//! `driftlab-cli` – diagnostics from the terminal.
//!
//! ```text
//! driftlab-cli list [--demo]
//! driftlab-cli monitor [--demo] [--seconds N]
//! driftlab-cli drift-test [--demo] [--seconds N]
//! driftlab-cli dump-calibration [--demo]
//! ```

use anyhow::{bail, Context, Result};
use driftlab_core::calibration::drift_test;
use driftlab_core::manager::{spawn_device, DeviceHandle, Event, Manager};
use driftlab_core::mock::MockDriver;
use driftlab_core::{Capabilities, ControllerInfo, Family};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Found {
    info: ControllerInfo,
    caps: Capabilities,
    handle: DeviceHandle,
}

fn discover(demo: bool) -> (Vec<Found>, crossbeam_channel::Receiver<Event>, Option<Manager>) {
    if demo {
        let (tx, rx) = crossbeam_channel::unbounded();
        spawn_device(Box::new(MockDriver::new(Family::DualSense)), tx, Arc::new(AtomicBool::new(false)));
        let mut v = vec![];
        if let Ok(Event::Added(info, caps, handle)) = rx.recv_timeout(Duration::from_secs(2)) {
            v.push(Found { info, caps, handle });
        }
        return (v, rx, None);
    }
    let m = Manager::start();
    let rx = m.events.clone();
    let mut v: Vec<Found> = vec![];
    let end = Instant::now() + Duration::from_millis(1800);
    while Instant::now() < end {
        if let Ok(Event::Added(info, caps, handle)) = rx.recv_timeout(Duration::from_millis(100)) {
            v.push(Found { info, caps, handle });
        }
    }
    (v, rx, Some(m))
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let demo = args.iter().any(|a| a == "--demo");
    let secs: u64 = arg_value(&args, "--seconds").and_then(|s| s.parse().ok()).unwrap_or(5);
    let Some(cmd) = args.first().map(String::as_str) else {
        println!("usage: driftlab-cli <list|monitor|drift-test|dump-calibration> [--demo] [--seconds N]");
        return Ok(());
    };
    match cmd {
        "list" => {
            let (found, _rx, _m) = discover(demo);
            if found.is_empty() {
                println!("No controllers found.");
            }
            for f in &found {
                println!(
                    "{} [{:04X}:{:04X}] {:?} {:?} fw={} serial={}",
                    f.info.name,
                    f.info.vid,
                    f.info.pid,
                    f.info.family,
                    f.info.connection,
                    f.info.firmware.as_deref().unwrap_or("?"),
                    f.info.masked_serial()
                );
                println!(
                    "  hw_calibration={} gyro={} touchpad={} rumble={} battery={}",
                    f.caps.hw_calibration, f.caps.gyro, f.caps.touchpad, f.caps.rumble, f.caps.battery
                );
            }
        }
        "monitor" => {
            let (found, rx, _m) = discover(demo);
            let f = found.first().context("no controller found")?;
            let end = Instant::now() + Duration::from_secs(secs);
            let mut last = Instant::now();
            while Instant::now() < end {
                if let Ok(Event::Input(id, st)) = rx.recv_timeout(Duration::from_millis(50)) {
                    if id == f.info.id && last.elapsed() > Duration::from_millis(100) {
                        last = Instant::now();
                        println!(
                            "L({:+.3},{:+.3}) R({:+.3},{:+.3}) L2={:.2} R2={:.2}",
                            st.left.0, st.left.1, st.right.0, st.right.1, st.l2, st.r2
                        );
                    }
                }
            }
        }
        "drift-test" => {
            let (found, _rx, _m) = discover(demo);
            let f = found.first().context("no controller found")?;
            println!("Leave both sticks alone for {secs} s…");
            let n = (secs * 200) as usize;
            let rep = f.handle.exec(move |d| drift_test(d, n)).recv_timeout(Duration::from_secs(secs + 15)).context("timeout")??;
            for (name, r) in [("left", rep.left), ("right", rep.right)] {
                println!(
                    "{name:>5}: offset ({:+.2} %, {:+.2} %)  noise {:.2} %  jitter {:.2} %  score {:.0}  → {:?}",
                    r.offset_x * 100.0,
                    r.offset_y * 100.0,
                    r.noise * 100.0,
                    r.jitter * 100.0,
                    r.score,
                    r.verdict
                );
            }
        }
        "dump-calibration" => {
            let (found, _rx, _m) = discover(demo);
            let f = found.first().context("no controller found")?;
            match f.handle.exec(|d| d.read_calibration()).recv_timeout(Duration::from_secs(10)).context("timeout")?? {
                Some(blob) => println!("{}", serde_json::to_string_pretty(&blob)?),
                None => bail!("this controller exposes no readable calibration data"),
            }
        }
        other => bail!("unknown command {other}"),
    }
    Ok(())
}
