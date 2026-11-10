use crate::device;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Pif {
    #[serde(with = "serde_big_array::BigArray")]
    pub ram: [u8; 64],
    pub channels: [PifChannel; 5],
}

#[derive(Copy, Clone, serde::Serialize, serde::Deserialize)]
pub struct PifChannel {
    pub tx: Option<usize>,
    pub tx_buf: Option<usize>,
    pub rx: Option<usize>,
    pub rx_buf: Option<usize>,
    #[serde(skip)]
    pub process: Option<fn(&mut device::Device, usize)>,
    pub pak_handler: Option<device::controller::PakHandler>,
    pub change_pak: device::controller::PakType,
}

pub const PIF_RAM_SIZE: usize = 64;
const PIF_CHANNELS_COUNT: usize = 5;
const PIF_RAM_OFFSET: usize = 0x7C0;
const PIF_MASK: usize = 0xFFFF;
const CHL_LEN: usize = 0x20;

pub fn read_mem(
    device: &mut device::Device,
    address: u64,
    _access_size: device::memory::AccessSize,
) -> u32 {
    device::cop0::add_cycles(device, 3000); //based on https://github.com/rasky/n64-systembench

    let mut masked_address = address as usize & PIF_MASK;
    if masked_address < PIF_RAM_OFFSET {
        // No boot ROM is carried (boot_hle stands in for it), so this reads as a locked-out ROM does.
        0
    } else {
        masked_address -= PIF_RAM_OFFSET;
        u32::from_be_bytes(
            device.pif.ram[masked_address..masked_address + 4]
                .try_into()
                .unwrap(),
        )
    }
}

pub fn write_mem(device: &mut device::Device, address: u64, value: u32, mask: u32) {
    let mut masked_address = address as usize & PIF_MASK;
    if masked_address < PIF_RAM_OFFSET {
        panic!("write to pif rom")
    }
    masked_address -= PIF_RAM_OFFSET;
    let mut data = u32::from_be_bytes(
        device.pif.ram[masked_address..masked_address + 4]
            .try_into()
            .unwrap(),
    );
    device::memory::masked_write_32(&mut data, value, mask);
    device.pif.ram[masked_address..masked_address + 4].copy_from_slice(&data.to_be_bytes());

    device.si.dma_dir = device::si::DmaDir::Write;
    device::events::create_event(device, device::events::EVENT_TYPE_SI, 3200); //based on https://github.com/rasky/n64-systembench
    device.si.regs[device::si::SI_STATUS_REG] |=
        device::si::SI_STATUS_DMA_BUSY | device::si::SI_STATUS_IO_BUSY
}

fn process_channel(device: &mut device::Device, channel: usize) -> usize {
    /* don't process channel if it has been disabled */
    if device.pif.channels[channel].tx.is_none() {
        return 0;
    }

    /* reset Tx/Rx just in case */
    device.pif.ram[device.pif.channels[channel].tx.unwrap()] &= 0x3f;
    device.pif.ram[device.pif.channels[channel].rx.unwrap()] &= 0x3f;

    /* set NoResponse if no device is connected */
    if device.pif.channels[channel].process.is_none() {
        device.pif.ram[device.pif.channels[channel].rx.unwrap()] |= 0x80;
        return 0;
    }

    /* do device processing */
    let process_handler = device.pif.channels[channel].process.unwrap();
    process_handler(device, channel);
    1
}

pub fn update_pif_ram(device: &mut device::Device) -> u64 {
    let mut active_channels = 0;
    for k in 0..PIF_CHANNELS_COUNT {
        active_channels += process_channel(device, k)
    }

    (24000 + (active_channels * 30000)) as u64
}

fn disable_pif_channel(channel: &mut PifChannel) {
    channel.tx = None;
    channel.rx = None;
    channel.tx_buf = None;
    channel.rx_buf = None;
}

fn setup_pif_channel(device: &mut device::Device, channel: usize, buf: usize) -> usize {
    let tx = device.pif.ram[buf] & 0x3f;
    let rx = device.pif.ram[buf + 1] & 0x3f;

    /* XXX: check out of bounds accesses */

    device.pif.channels[channel].tx = Some(buf);
    device.pif.channels[channel].rx = Some(buf + 1);
    device.pif.channels[channel].tx_buf = Some(buf + 2);
    device.pif.channels[channel].rx_buf = Some(buf + 2 + tx as usize);

    (2 + tx + rx) as usize
}

fn setup_channels_format(device: &mut device::Device) {
    let mut i: usize = 0;
    let mut k: usize = 0;
    while i < PIF_RAM_SIZE && k < PIF_CHANNELS_COUNT {
        match device.pif.ram[i] {
            0x00 => {
                /* skip channel */
                disable_pif_channel(&mut device.pif.channels[k]);
                k += 1;
                i += 1;
            }

            0xff => {
                /* dummy data */
                i += 1;
            }

            0xfe => {
                /* end of channel setup - remaining channels are disabled */
                while k < PIF_CHANNELS_COUNT {
                    disable_pif_channel(&mut device.pif.channels[k]);
                    k += 1;
                }
            }

            0xfd => {
                /* channel reset - send reset command and discard the results */
                disable_pif_channel(&mut device.pif.channels[k]); // not sure about this
                k += 1;
                i += 1;
            }

            _ => {
                /* setup channel */

                /* HACK?: some games sends bogus PIF commands while accessing controller paks
                 * Yoshi Story, Top Gear Rally 2, Indiana Jones, ...
                 * When encountering such commands, we skip this bogus byte.
                 */
                if (i + 1 < PIF_RAM_SIZE) && (device.pif.ram[i + 1] == 0xfe) {
                    i += 1;
                    continue;
                }

                if (i + 2) >= PIF_RAM_SIZE {
                    i = PIF_RAM_SIZE;
                    continue;
                }

                i += setup_pif_channel(device, k, i);
                k += 1;
            }
        }
    }
}

pub fn process_ram(device: &mut device::Device) {
    let mut clrmask = 0x00;
    let command = device.pif.ram[0x3F];
    if command & 0x01 != 0 {
        // Configure joybus protocol
        setup_channels_format(device);
        clrmask |= 0x01
    }
    if command & 0x02 != 0 {
        // Challenge / response for protection
        /* disable channel processing when doing CIC challenge */
        for k in 0..PIF_CHANNELS_COUNT {
            disable_pif_channel(&mut device.pif.channels[k]);
        }

        /* CIC Challenge */
        process_cic_challenge(device);
        clrmask |= 0x02;
    }
    if command & 0x08 != 0 {
        // Terminate boot process
        clrmask |= 0x08;
    }
    if command & 0x20 != 0 {
        // Acquire checksum
        device.pif.ram[0x3F] = 0x80
    }
    device.pif.ram[0x3F] &= !clrmask
}

pub fn connect_pif_channels(device: &mut device::Device) {
    for i in 0..4 {
        if let Some(netplay) = &device.netplay {
            if netplay.connected[i] {
                device.pif.channels[i].process = Some(device::controller::process);
            }
        } else if device.ui.config.input.controller_enabled[i] {
            device.pif.channels[i].process = Some(device::controller::process);
        }
    }
    if device.ui.config.input.emulate_vru && device.netplay.is_none() {
        device.pif.channels[3].process = Some(device::controller::vru::process);
    }
    device.pif.channels[4].process = Some(device::cart::process)
}

fn get_default_handler(device: &device::Device) -> device::controller::PakHandler {
    if device.ui.game_id == "NCT" {
        // Chameleon Twist does not support the mempak
        device::controller::PakHandler {
            read: device::controller::rumble::read,
            write: device::controller::rumble::write,
            pak_type: device::controller::PakType::RumblePak,
        }
    } else {
        device::controller::PakHandler {
            read: device::controller::mempak::read,
            write: device::controller::mempak::write,
            pak_type: device::controller::PakType::MemPak,
        }
    }
}

pub fn reset_pif(device: &mut device::Device, is_nmi_reset: bool) {
    let reset_type = u32::from(is_nmi_reset);
    let rom_type = 0u32;
    let s7 = 0u32;
    let cic = u32::from(device.cart.cic_seed);
    let word = (rom_type << 19) | (s7 << 18) | (reset_type << 17) | (cic << 8) | 0x3f;
    device.pif.ram[0x24..0x28].copy_from_slice(&word.to_be_bytes());
}

// Boots by high-level emulation instead of running Nintendo's PIF boot ROM: leaves the CPU and the RCP as the boot
// ROM would and starts the cart's own IPL3 in DMEM. Follows mupen64plus-core's src/device/pif/bootrom_hle.c
// ("Copyright (C) 2016 Bobby Smiles", GPL-2.0-or-later) at commit a9ca8ec0b7decd17390c76c6341cf423d5d79321.
// Expects reset_pif to have written the PIF RAM word at 0x24.
pub fn boot_hle(device: &mut device::Device) {
    // Register byte offsets within each interface's block.
    const SP_STATUS: u64 = 4 * 4;
    const PI_STATUS: u64 = 4 * 4;
    const PI_BSD_DOM1_LAT: u64 = 5 * 4;
    const PI_BSD_DOM1_PWD: u64 = 6 * 4;
    const PI_BSD_DOM1_PGS: u64 = 7 * 4;
    const PI_BSD_DOM1_RLS: u64 = 8 * 4;
    const VI_V_INTR: u64 = 3 * 4;
    const VI_CURRENT: u64 = 4 * 4;
    const VI_H_START: u64 = 9 * 4;
    const AI_DRAM_ADDR: u64 = 0;
    const AI_LEN: u64 = 4;

    device.cpu.cop0.regs[device::cop0::COP0_STATUS_REG] = 0x34000000;
    device.cpu.cop0.regs[device::cop0::COP0_CONFIG_REG] = 0x7006e463;

    // halt the RSP and clear its interrupt, reset the PI, blank the screen, mute the sound
    device::rsp_interface::write_regs(device, SP_STATUS, 0x0a, u32::MAX);
    device::pi::write_regs(device, PI_STATUS, 0x03, u32::MAX);
    device::vi::write_regs(device, VI_V_INTR, 1023, u32::MAX);
    device::vi::write_regs(device, VI_CURRENT, 0, u32::MAX);
    device::vi::write_regs(device, VI_H_START, 0, u32::MAX);
    device::ai::write_regs(device, AI_DRAM_ADDR, 0, u32::MAX);
    device::ai::write_regs(device, AI_LEN, 0, u32::MAX);

    // s3-s7, which the game's OS reads
    let pif24 = u32::from_be_bytes(device.pif.ram[0x24..0x28].try_into().unwrap());
    device.cpu.gpr[19] = u64::from((pif24 >> 19) & 0x01); // ROM type: 0 for a cart
    device.cpu.gpr[20] = if device.cart.pal { 0 } else { 1 }; // TV type
    device.cpu.gpr[21] = u64::from((pif24 >> 17) & 0x01); // reset type: 1 after a soft reset
    device.cpu.gpr[22] = u64::from((pif24 >> 8) & 0xff); // CIC seed
    device.cpu.gpr[23] = u64::from((pif24 >> 18) & 0x01);

    // the cart's ROM timing, from its first word
    let mut header = [0u8; 0x1000];
    let available = device.cart.rom.len().min(header.len());
    header[..available].copy_from_slice(&device.cart.rom[..available]);
    let bsd_dom1_config = u32::from_be_bytes(header[0..4].try_into().unwrap());
    device::pi::write_regs(device, PI_BSD_DOM1_LAT, bsd_dom1_config & 0xff, u32::MAX);
    device::pi::write_regs(
        device,
        PI_BSD_DOM1_PWD,
        (bsd_dom1_config >> 8) & 0xff,
        u32::MAX,
    );
    device::pi::write_regs(
        device,
        PI_BSD_DOM1_PGS,
        (bsd_dom1_config >> 16) & 0x0f,
        u32::MAX,
    );
    device::pi::write_regs(
        device,
        PI_BSD_DOM1_RLS,
        (bsd_dom1_config >> 20) & 0x03,
        u32::MAX,
    );

    // the cart's IPL3, into DMEM
    device.rsp.mem[0x40..0x1000].copy_from_slice(&header[0x40..0x1000]);

    // What the boot ROM's IPL2 leaves at the start of IMEM, as bootrom_hle.c writes it ("required by CIC x105"). The
    // CIC-6105 IPL3 checks these 32 bytes exactly: an equivalent instruction in place of one of them stops Ocarina of
    // Time booting. Kept on the user's decision of 2026-11-10 (Rombie's .docs/licenses.md, legal question 14).
    for (i, word) in [
        0x3c0dbfc0u32,
        0x8da807fc,
        0x25ad07c0,
        0x31080080,
        0x5500fffc,
        0x3c0dbfc0,
        0x8da80024,
        0x3c0bb000,
    ]
    .into_iter()
    .enumerate()
    {
        device::rsp_interface::write_mem(device, 0x1000 + 4 * i as u64, word, u32::MAX);
    }

    device.cpu.gpr[11] = device::cpu_instructions::se32(0xa4000040u32 as i32); // t3
    device.cpu.gpr[29] = device::cpu_instructions::se32(0xa4001ff0u32 as i32); // sp
    device.cpu.gpr[31] = device::cpu_instructions::se32(0xa4001550u32 as i32); // ra
    device.cpu.pc = device::cpu_instructions::se32(0xa4000040u32 as i32);
    device.cpu.branch_state.state = device::cpu::State::Step;
}

pub fn init(device: &mut device::Device) {
    reset_pif(device, false);

    let default_handler = get_default_handler(device);
    let tpak_handler = device::controller::PakHandler {
        read: device::controller::transferpak::read,
        write: device::controller::transferpak::write,
        pak_type: device::controller::PakType::TransferPak,
    };

    connect_pif_channels(device);

    for i in 0..4 {
        if let Some(netplay) = &device.netplay {
            if netplay.connected[i] {
                device.pif.channels[i].pak_handler = Some(default_handler);
            }
        } else if device.ui.config.input.controller_enabled[i] {
            if device.transferpaks[i].cart.rom.is_empty() {
                device.pif.channels[i].pak_handler = Some(default_handler);
            } else {
                device.pif.channels[i].pak_handler = Some(tpak_handler);
            }
        }
    }
    if device.ui.config.input.emulate_vru && device.netplay.is_none() {
        device.pif.channels[3].pak_handler = None;
    }
}

fn process_cic_challenge(device: &mut device::Device) {
    let mut challenge: [u8; 30] = [0; 30];
    let mut response: [u8; 30] = [0; 30];

    /* format the 'challenge' message into 30 nibbles for X-Scale's CIC code */
    for i in 0..15 {
        challenge[i * 2] = (device.pif.ram[0x30 + i] >> 4) & 0x0f;
        challenge[i * 2 + 1] = device.pif.ram[0x30 + i] & 0x0f;
    }

    /* calculate the proper response for the given challenge (X-Scale's algorithm) */
    n64_cic_nus_6105(challenge, &mut response, CHL_LEN - 2);
    device.pif.ram[0x2e] = 0;
    device.pif.ram[0x2f] = 0;

    /* re-format the 'response' into a byte stream */
    for i in 0..15 {
        device.pif.ram[0x30 + i] = (response[i * 2] << 4) + response[i * 2 + 1];
    }
}

fn n64_cic_nus_6105(chl: [u8; 30], rsp: &mut [u8; 30], len: usize) {
    let lut0: [u8; 0x10] = [
        0x4, 0x7, 0xA, 0x7, 0xE, 0x5, 0xE, 0x1, 0xC, 0xF, 0x8, 0xF, 0x6, 0x3, 0x6, 0x9,
    ];
    let lut1: [u8; 0x10] = [
        0x4, 0x1, 0xA, 0x7, 0xE, 0x5, 0xE, 0x1, 0xC, 0x9, 0x8, 0x5, 0x6, 0x3, 0xC, 0x9,
    ];

    let mut key = 0xB;
    let mut lut = lut0.as_ref();
    for i in 0..len {
        rsp[i] = (key + 5 * chl[i]) & 0xF;
        key = lut[rsp[i] as usize];
        let sgn = (rsp[i] >> 3) & 0x1;
        let mut mag;
        if sgn == 1 {
            mag = !rsp[i]
        } else {
            mag = rsp[i]
        }
        mag &= 0x7;
        let mut modd;
        if mag % 3 == 1 {
            modd = sgn
        } else {
            modd = 1 - sgn;
        }
        if lut == lut1 && (rsp[i] == 0x1 || rsp[i] == 0x9) {
            modd = 1;
        }
        if lut == lut1 && (rsp[i] == 0xB || rsp[i] == 0xE) {
            modd = 0;
        }
        if modd == 1 {
            lut = &lut1;
        } else {
            lut = &lut0;
        }
    }
}
