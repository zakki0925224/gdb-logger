use config::Config;
use core::time::Duration;
use debug_info::{DebugInfo, Register};
use gdbmi::Gdb;
use nix::libc::{SIGINT, kill};
use std::{fs, process::Stdio};
use tokio::{net::UdpSocket, time::sleep};

use crate::debug_info::DiffInfo;

mod config;
mod debug_info;

const CONFIG_PATH: &str = "./config.toml";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // parse config
    let config: Config = toml::from_str(&fs::read_to_string(CONFIG_PATH)?)?;
    println!("Parsed configurations: {:?}", config);

    // create udp server
    let udp_addr = format!("0.0.0.0:{}", config.udp_port);
    let udp_socket = UdpSocket::bind(&udp_addr).await?;
    println!("Waiting for UDP client on port {}...", config.udp_port);

    let mut buf = [0u8; 64];
    let (_, client_addr) = udp_socket.recv_from(&mut buf).await?;
    println!("UDP client connected: {}", client_addr);

    let duration = Duration::from_secs_f32(config.gdb_timeout);
    let wait_booting = Duration::from_secs_f32(config.gdb_timeout_wait_booting);
    let wait_next = Duration::from_secs_f32(config.gdb_timeout_wait_next);

    let cmd = tokio::process::Command::new("gdb")
        .arg("--interpreter=mi3")
        .arg(config.gdb_symbol_file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let pid = cmd.id().unwrap();

    let mut gdb = Gdb::new(cmd, duration);

    println!("Connecting to GDB server on port {}...", config.gdb_port);
    gdb.raw_cmd(format!("target remote :{}", config.gdb_port))
        .await?;
    println!("Connected to GDB server");
    // can insert breakpoints here

    gdb.set_timeout(wait_booting);
    let _ = gdb.exec_continue().await; // timeout
    sleep(wait_booting).await;
    unsafe {
        kill(pid as i32, SIGINT); // send Ctrl-C
    }
    gdb.set_timeout(duration);
    let _ = gdb.exec_continue().await;

    let mut old_debug_info = None;

    loop {
        sleep(wait_next).await;
        unsafe {
            kill(pid as i32, SIGINT); // send Ctrl-C
        }

        let payload = gdb
            .raw_cmd("-data-list-register-values x")
            .await?
            .expect_result()?
            .expect_payload()?;
        let register_values = &payload.as_map()["register-values"];
        let list = register_values.clone().expect_list()?;

        let mut regs: Vec<Register> = Vec::new();
        for v in list.iter() {
            let dict_data = v.clone().expect_dict()?;
            let dict = dict_data.as_map();
            let number: u8 = dict["number"].clone().expect_string()?.parse()?;
            let value = match u64::from_str_radix(&dict["value"].clone().expect_string()?[2..], 16)
            {
                Ok(v) => v,
                Err(_) => {
                    continue;
                }
            };
            let reg: Register = (number, value).into();
            if !reg.is_other() {
                regs.push(reg);
            }
        }

        let frame = gdb.stack_info_frame().await?;
        let variables = gdb.stack_list_variables(false).await?;

        let new_debug_info = DebugInfo {
            regs,
            frame: frame.into(),
            variables: variables.into_iter().map(|v| v.into()).collect(),
        };

        let diff_info = if let Some(old_debug_info) = &old_debug_info {
            debug_info::diff_debug_info(old_debug_info, &new_debug_info)
        } else {
            old_debug_info = Some(new_debug_info.clone());
            DiffInfo {
                changed_regs: new_debug_info.regs,
                frame: Some(new_debug_info.frame),
                variables: Some(new_debug_info.variables),
            }
        };
        let json = serde_json::to_string(&diff_info)?;

        // send to udp client
        if let Err(e) = udp_socket.send_to(json.as_bytes(), &client_addr).await {
            println!("UDP send error: {}", e);
            break;
        }

        if gdb.exec_continue().await.is_err() {
            println!("Error stepping through the code");
            break;
        }
    }
    gdb.exec_finish().await?;

    Ok(())
}
