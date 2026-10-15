//! Explicit, read-only Windows clock diagnostic. Never adjusts the system clock.
use serde::Serialize;
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub checked_ms: u64,
    pub source: &'static str,
    pub service_status_readable: bool,
    pub samples: usize,
    pub median_offset_ms: Option<f64>,
    pub spread_ms: Option<f64>,
    pub state: &'static str,
    pub detail: &'static str,
}

fn report(status_readable: bool, output: &str) -> Report {
    // w32tm prints a signed offset followed by s. Error rows are not samples.
    let mut samples: Vec<f64> = output
        .lines()
        .filter_map(|line| {
            let last = line.split_whitespace().last()?;
            if !last.starts_with(['+', '-']) {
                return None;
            }
            let number = last.strip_suffix('s')?.replace(',', ".");
            number
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite() && n.abs() < 86400.)
                .map(|n| n * 1000.)
        })
        .collect();
    samples.sort_by(f64::total_cmp);
    let count = samples.len();
    let median = (count == 3).then(|| samples[1]);
    let spread = median.map(|_| samples[2] - samples[0]);
    let (state,detail)=match (median,spread){
        (None,_) => ("UNVERIFIED","No se recibieron tres muestras válidas. Revisa el acceso NTP; no se acredita sincronización."),
        (Some(_),Some(s)) if s>100. =>("UNSTABLE","Las muestras varían más de 100 ms; la comparación no es estable."),
        (Some(m),_) if m.abs()>100. =>("OFFSET_DETECTED","Desfase superior a 100 ms. Revisar sincronización antes de certificar latencia o frescura."),
        _ =>("SMALL_OBSERVED_OFFSET","Desfase observado de hasta 100 ms. Es una comprobación puntual, no una garantía de sincronización."),
    };
    Report {
        checked_ms: crate::live::now_ms(),
        source: "time.windows.com",
        service_status_readable: status_readable,
        samples: count,
        median_offset_ms: median,
        spread_ms: spread,
        state,
        detail,
    }
}

fn run(args: &[&str]) -> Result<(bool, String), &'static str> {
    let root = std::env::var_os("SystemRoot").ok_or("Windows no disponible")?;
    let exe = std::path::PathBuf::from(root).join("System32/w32tm.exe");
    let mut command = Command::new(exe);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|_| "No se pudo iniciar el diagnóstico de reloj")?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < Duration::from_secs(10) => {
                std::thread::sleep(Duration::from_millis(25))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("La comprobación del reloj agotó su plazo");
            }
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|_| "No se pudo leer el diagnóstico del reloj")?;
    Ok((
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    ))
}
pub fn inspect() -> Result<Report, &'static str> {
    if !cfg!(windows) {
        return Err("Diagnóstico disponible en Windows");
    }
    let (status, _) = run(&["/query", "/status"])?;
    let (_, samples) = run(&[
        "/stripchart",
        "/computer:time.windows.com",
        "/samples:3",
        "/period:1",
        "/dataonly",
    ])?;
    Ok(report(status, &samples))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offsets_are_localized_bounded_and_do_not_invent_success_from_error_rows() {
        let r = report(
            false,
            "20:47:17, +02.8032308s\n20:47:18, +02.8167901s\n20:47:19, +02.8063808s",
        );
        assert_eq!(r.state, "OFFSET_DETECTED");
        assert!(!r.service_status_readable);
        assert!((r.median_offset_ms.unwrap() - 2806.3808).abs() < 0.0001);
        let r = report(
            true,
            "10:00:00, -00,020s\n10:00:01, -00,030s\n10:00:02, -00,010s",
        );
        assert_eq!(r.median_offset_ms, Some(-20.));
        assert_eq!(r.state, "SMALL_OBSERVED_OFFSET");
        for input in [
            "error: 0x800705B4",
            "+NaNs\n+infs\n+90000s",
            "+0.001s\n+0.002s",
        ] {
            let r = report(true, input);
            assert_eq!(r.state, "UNVERIFIED");
            assert_eq!(r.median_offset_ms, None);
        }
        assert_eq!(report(true, "+0.010s\n+0.030s\n+0.200s").state, "UNSTABLE");
    }
}
