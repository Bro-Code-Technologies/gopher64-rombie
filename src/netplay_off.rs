// Stands in for netplay.rs when the `upstream-extras` feature is off, as in the Rombie engine build, so gopher64's own
// netplay (ggrs over matchbox WebRTC) isn't compiled. `Netplay` can't be constructed, so `device.netplay` is always
// None and nothing here runs.
use crate::{device, ui};

#[allow(dead_code)] // built from the command line, then ignored by init
pub struct NetplayConfig {
    pub server_addr: String,
    pub player_number: usize,
    pub number_of_players: usize,
    pub input_delay: usize,
    pub ice_config_path: std::path::PathBuf,
}

#[allow(dead_code)] // never constructed, by design
pub struct Netplay {
    pub player_number: usize,
    pub connected: [bool; 4],
    pub input_delay: usize,
    pub inputs: Vec<(ui::input::InputData, ())>,
    never: std::convert::Infallible,
}

pub fn send_rtc(netplay: &mut Netplay, _rtc: i64) {
    match netplay.never {}
}

pub fn receive_rtc(netplay: &mut Netplay) -> i64 {
    match netplay.never {}
}

pub fn send_rng(netplay: &mut Netplay, _seed: u64) {
    match netplay.never {}
}

pub fn receive_rng(netplay: &mut Netplay) -> u64 {
    match netplay.never {}
}

pub fn send_save(netplay: &mut Netplay, _save_type: &str, _save_data: &[u8]) {
    match netplay.never {}
}

pub fn receive_save(netplay: &mut Netplay, _save_type: &str, _save_data: &mut Vec<u8>) {
    match netplay.never {}
}

pub fn send_input_delay(netplay: &mut Netplay, _input_delay: usize) {
    match netplay.never {}
}

pub fn in_rollback(netplay: Option<&Netplay>) -> bool {
    match netplay {
        Some(netplay) => match netplay.never {},
        None => false,
    }
}

pub fn process_requests(device: &mut device::Device) -> Vec<(ui::input::InputData, ())> {
    match &device.netplay {
        Some(netplay) => match netplay.never {},
        None => Vec::new(),
    }
}

pub fn init(
    _device: &mut device::Device,
    _netplay_config: &NetplayConfig,
    _pal: bool,
) -> Option<Netplay> {
    None
}

pub fn close(netplay: &mut Netplay) {
    match netplay.never {}
}
