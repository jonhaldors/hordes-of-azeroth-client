use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use benilla_mlua::{Lua, Result as LuaResult};
use benilla_protocol::{current_gate, set_gate, Gate};

const DEFAULT_AGENT_PORT: u16 = 3727;
const CONNECT_TIMEOUT: Duration = Duration::from_millis(300);
const ANSWER_TIMEOUT: Duration = Duration::from_millis(1500);
const ANSWER_LIMIT: u64 = 256 * 1024;

static JOINED_LOGIN: Mutex<Option<(String, String)>> = Mutex::new(None);

pub fn register_coop_api(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();
    globals.set(
        "HoaCoop_Call",
        lua.create_function(|_, (method, path, body): (String, String, Option<String>)| {
            Ok(match call_agent(&method, &path, body.as_deref().unwrap_or("")) {
                Ok(answer) => (Some(answer), None),
                Err(error) => (None, Some(error)),
            })
        })?,
    )?;
    globals.set(
        "HoaCoop_SetGate",
        lua.create_function(
            |_, (address, token, user, password): (Option<String>, Option<String>, Option<String>, Option<String>)| {
                let joined = match (address, token, user, password) {
                    (Some(address), Some(token), Some(user), Some(password))
                        if !address.is_empty() && !token.is_empty() =>
                    {
                        Some((Gate { address, token }, user, password))
                    }
                    _ => None,
                };
                set_gate(joined.as_ref().map(|(gate, _, _)| gate.clone()));
                *JOINED_LOGIN.lock().unwrap_or_else(PoisonError::into_inner) =
                    joined.map(|(_, user, password)| (user, password));
                Ok(())
            },
        )?,
    )?;
    globals.set(
        "HoaCoop_GetGate",
        lua.create_function(|_, ()| {
            let login = JOINED_LOGIN.lock().unwrap_or_else(PoisonError::into_inner).clone();
            Ok(match (current_gate(), login) {
                (Some(gate), Some((user, password))) => (Some(gate.address), Some(user), Some(password)),
                _ => (None, None, None),
            })
        })?,
    )?;
    Ok(())
}

fn agent_port() -> u16 {
    std::env::var("HOA_COOP_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(DEFAULT_AGENT_PORT)
}

fn call_agent(method: &str, path: &str, body: &str) -> Result<String, String> {
    let address = SocketAddr::from(([127, 0, 0, 1], agent_port()));
    let mut stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)
        .map_err(|_| "The launcher isn't running.".to_string())?;
    stream
        .set_read_timeout(Some(ANSWER_TIMEOUT))
        .map_err(|e| e.to_string())?;
    let request = format!(
        "{method} {path} HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    stream
        .take(ANSWER_LIMIT)
        .read_to_end(&mut raw)
        .map_err(|_| "The launcher didn't answer.".to_string())?;
    let text = String::from_utf8_lossy(&raw);
    let (head, content) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| "The launcher sent a broken answer.".to_string())?;
    let ok = head
        .lines()
        .next()
        .is_some_and(|status| status.split_whitespace().nth(1) == Some("200"));
    if ok {
        Ok(content.to_string())
    } else {
        Err(content.trim().to_string())
    }
}
