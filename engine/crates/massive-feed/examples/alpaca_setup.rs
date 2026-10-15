//! One-use loopback form. Secrets go directly to Windows DPAPI, never stdout or source.
use massive_feed::alpaca::Credentials;
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let token = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    );
    println!("Abre http://{address}/{token}");
    for stream in listener.incoming() {
        let mut stream = stream?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut bytes = Vec::new();
        let mut buf = [0; 4096];
        let header_end = loop {
            let n = stream.read(&mut buf)?;
            if n == 0 {
                return Err("Conexión incompleta".into());
            }
            bytes.extend_from_slice(&buf[..n]);
            if bytes.len() > 16384 {
                return Err("Solicitud demasiado grande".into());
            }
            if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                break i + 4;
            }
        };
        let headers = String::from_utf8_lossy(&bytes[..header_end]).to_string();
        let first = headers.lines().next().unwrap_or("");
        let host = format!("127.0.0.1:{}", address.port());
        if !headers
            .lines()
            .any(|l| l.eq_ignore_ascii_case(&format!("Host: {host}")))
        {
            continue;
        }
        let mut success = false;
        let (status, body) = if first == format!("GET /{token} HTTP/1.1") {
            ("200 OK",format!("<!doctype html><meta charset=utf-8><title>Conectar Alpaca Paper a Snipers</title><h1>Guardar Alpaca Paper en Snipers</h1><p>Almacén cifrado de Windows. Solo simulación.</p><form method=post action='/{token}'><label>API Key <input name=key type=password autocomplete=off required></label><label>Secret Key <input name=secret type=password autocomplete=off required></label><button>Guardar claves Paper cifradas</button></form>"))
        } else if first == format!("POST /{token} HTTP/1.1")
            && headers
                .lines()
                .any(|l| l.eq_ignore_ascii_case(&format!("Origin: http://{host}")))
        {
            let length = headers
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            if length > 4096 {
                return Err("Formulario demasiado grande".into());
            }
            while bytes.len() < header_end + length {
                let n = stream.read(&mut buf)?;
                if n == 0 {
                    return Err("Formulario incompleto".into());
                }
                bytes.extend_from_slice(&buf[..n]);
            }
            let body = String::from_utf8_lossy(&bytes[header_end..header_end + length]);
            let url = reqwest::Url::parse(&format!("http://localhost/?{body}"))?;
            let fields: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
            let result = Credentials::new(
                fields.get("key").cloned().unwrap_or_default(),
                fields.get("secret").cloned().unwrap_or_default(),
            )
            .and_then(|c| c.save());
            if result.is_ok() {
                success = true;
                ("200 OK","<h1>Credenciales Paper guardadas con cifrado de Windows.</h1><p>Puedes cerrar esta pestaña.</p>".to_owned())
            } else {
                (
                    "400 Bad Request",
                    "<h1>No se pudieron guardar las claves Paper.</h1>".to_owned(),
                )
            }
        } else {
            ("404 Not Found", "No disponible".into())
        };
        write!(stream,"HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'; form-action 'self'; frame-ancestors 'none'\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",body.len())?;
        if success {
            println!("Credenciales cifradas guardadas; servidor cerrado.");
            break;
        }
    }
    Ok(())
}
