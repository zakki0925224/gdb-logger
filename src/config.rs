use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub gdb_port: u16,
    pub gdb_timeout: f32,
    pub gdb_timeout_wait_booting: f32,
    pub gdb_timeout_wait_next: f32,
    pub gdb_symbol_file: String,

    pub udp_port: u16,
}
