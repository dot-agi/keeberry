// SPDX-License-Identifier: GPL-2.0-or-later
#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use panic_probe as _;

mod behavior;
mod boot;
mod clock;
mod config;
mod digitizer;
mod features;
mod flash;
mod gamepad;
mod gpio;
mod kcp;
mod keycode;
mod keymap;
mod matrix;
mod midi;
mod mouse;
mod rgb;
mod telemetry;
mod timed;
mod time_driver;
mod uart;
mod usb;
mod wireless;
mod xinput;

/// Entry point: bring up the clock and the TIM2 time base, initialise the
/// matrix, then hand control to the embassy executor running [`usb::usb_task`],
/// which drives the USB device, the keyboard report loop, the shared report-ID
/// loop (NKRO + consumer + system) and the kcp config-protocol loop concurrently.
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = pac::Peripherals::take().unwrap();

    clock::init(&p);
    defmt::info!("keeberry: system clock up (HCLK = {} Hz)", clock::HCLK_HZ);

    time_driver::init(&p);
    defmt::info!(
        "keeberry: TIM2 embassy-time driver up ({} Hz tick)",
        embassy_time::TICK_HZ
    );

    usb::wb32_usb_init(&p);
    defmt::info!("keeberry: USB controller clocked + PHY/D-/D+ up");

    matrix::init(&p);
    defmt::info!(
        "keeberry: matrix ready ({=usize}x{=usize}, ROW2COL)",
        matrix::NUM_ROWS,
        matrix::NUM_COLS
    );

    rgb::init(&p);
    defmt::info!(
        "keeberry: RGB ready (WS2812-over-SPIM2, {=usize} LEDs on PB15)",
        rgb::LED_COUNT
    );

    // Bring up the CH582 radio transport (UART3): the link, framed send/receive
    // and stop-and-wait delivery. Report routing, the power-on init burst,
    // pairing and the kcp-over-radio bridge run in the wireless tasks spawned
    // below.
    wireless::init(&p);
    defmt::info!("keeberry: wireless transport up (UART3 @ 115200 8N1, PC10/PC11)");

    // Restore the complete saved configuration from flash, if one is stored. A
    // valid blob (magic + version + CRC) overwrites every group's RAM state —
    // keymap, NKRO, RGB, SOCD, overrides, tap-dance, combos and macros; otherwise
    // the power-on defaults stand. Persisting is host-driven via the kcp CONFIG
    // group (SAVE); this is the matching restore path.
    if config::restore() {
        defmt::info!("keeberry: restored saved configuration from flash");
    } else {
        defmt::info!("keeberry: no valid saved configuration; using defaults");
    }

    // `usb_task` joins the USB device with the keyboard report loop (matrix
    // scan -> keymap engine -> HID) and the kcp config-protocol loop (raw-HID),
    // so no separate scan task is needed.
    spawner.must_spawn(usb::usb_task());

    // The RGB render loop is independent of USB: its own ~30 Hz task driving
    // the WS2812 chain over SPIM2, reading the live state the kcp RGB group sets.
    spawner.must_spawn(rgb::rgb_task());

    // Radio transport pumps: the RX state machine and the stop-and-wait TX
    // driver. They keep the link serviced; report routing (in `usb_task`), the
    // kcp-over-radio bridge and the connection state machine sit on top.
    spawner.must_spawn(wireless::rx_task());
    spawner.must_spawn(wireless::tx_task());

    // Bridge the host's raw-HID config traffic forwarded by the 2.4G dongle into
    // the same kcp dispatcher that serves USB, and frame the replies back.
    spawner.must_spawn(wireless::kcp_radio_task());

    // The power-on init burst (firmware version, sleep policy, initial transport
    // select) plus the periodic battery / charge cadence.
    spawner.must_spawn(wireless::housekeeping_task());

    // The auto-fallback transport supervisor: selects the best available transport
    // (USB > 2.4 GHz > BT1/2/3), switching to USB on a cable plug and falling back to
    // the preferred wireless — walking the priority order — on unplug or link loss.
    spawner.must_spawn(wireless::transport_supervisor_task());
}
