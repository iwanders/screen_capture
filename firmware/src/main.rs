//! This example is a mix of:
//! https://github.com/stm32-rs/stm32f1xx-hal/blob/f9b24f4d9bac7fc3c93764bd295125800944f53b/examples/blinky.rs
//! and
//! https://github.com/stm32-rs/stm32f1xx-hal/blob/f9b24f4d9bac7fc3c93764bd295125800944f53b/examples/usb_serial.rs
//!
//! This assumes that a LED is connected to pc13 as is the case on the blue pill board.
//!
//! Note: Without additional hardware, PC13 should not be used to drive an LED, see page 5.1.2 of
//! the reference manual for an explanation. This is not an issue on the blue pill.

// https://github.com/adamgreig/ledeaf/blob/fbfed437c77f9bc4d83ea9fae4cee4e107af2e15/firmware/src/main.rs
// https://github.com/thalesfragoso/keykey/blob/master/keykey/Cargo.toml
// https://github.com/rtic-rs/cortex-m-rtic

// #![deny(unsafe_code)]

#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
use panic_halt as _;

use cortex_m::asm::delay;
use cortex_m_rt::entry;
use stm32f1xx_hal::prelude::*; //, timer::Timer

// use embedded_hal::digital::v2::OutputPin;
// use embedded_hal::digital::v2::PinState::{High, Low};

use stm32f1xx_hal::pac::{self}; // , interrupt, Interrupt, NVIC
                                // use stm32f1xx_hal::prelude::*;
use stm32f1xx_hal::usb::Peripheral;

// use cortex_m_rt::entry;
// use displaylight_fw::ringbuffer;
use displaylight_fw::serial;
// use displaylight_fw::spsc;
use displaylight_fw::string;


use stm32f1xx_hal::{
    spi::{Mode, Phase, Polarity, Spi},
};



#[repr(C, packed)]
#[derive(Default, Copy, Clone)]
/// Struct to represent the RGB state of a single led.
pub struct RGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}



use cortex_m::{singleton};

static mut G_V: usize = 0;

#[cfg_attr(not(test), entry)]
fn main() -> ! {
    // Get access to the core peripherals from the cortex-m crate
    let _cp = cortex_m::Peripherals::take().unwrap();
    // Get access to the device specific peripherals from the peripheral access crate
    let dp = pac::Peripherals::take().unwrap();

    // Take ownership over the raw flash and rcc devices and convert them into the corresponding
    // HAL structs
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();

    // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
    // `clocks`
    // let clocks = rcc.cfgr.freeze(&mut flash.acr);
    // Set a real clock that allows usb.
    let clocks = rcc
        .cfgr
        .use_hse(8.MHz())
        .sysclk(48.MHz())
        .pclk1(24.MHz())
        .freeze(&mut flash.acr);

    assert!(clocks.usbclk_valid());

    // Acquire the GPIOC peripheral
    let mut gpioc = dp.GPIOC.split();

    // Configure gpio C pin 13 as a push-pull output. The `crh` register is passed to the function
    // in order to configure the port. For pins 0-7, crl should be passed instead.
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
    // Configure the syst timer to trigger an update every second
    // let mut timer = Timer::syst(cp.SYST, &clocks).counter_hz();
    // timer.start(5.Hz()).unwrap();

    // Setup usb serial

    let mut gpioa = dp.GPIOA.split();

    // BluePill board has a pull-up resistor on the D+ line.
    // Pull the D+ pin down to send a RESET condition to the USB bus.
    // This forced reset is needed only for development, without it host
    // will not reset your device when you upload new firmware.
    let mut usb_dp = gpioa.pa12.into_push_pull_output(&mut gpioa.crh);
    usb_dp.set_low();
    delay(clocks.sysclk().raw() / 100);

    let usb_dm = gpioa.pa11;
    let usb_dp = usb_dp.into_floating_input(&mut gpioa.crh);

    let usb = Peripheral {
        usb: dp.USB,
        pin_dm: usb_dm,
        pin_dp: usb_dp,
    };

    let mut s = serial::Serial::new(usb);



    // https://github.com/stm32-rs/stm32f1xx-hal/blob/f9b24f4d9bac7fc3c93764bd295125800944f53b/examples/spi-dma.rs
    // https://github.com/stm32-rs/stm32f1xx-hal/blob/f9b24f4d9bac7fc3c93764bd295125800944f53b/examples/adc-dma-circ.rs
    // We want an SPI transaction that just keeps writing bytes on the port.
    // 
    // spi on bus B
    let mut gpiob = dp.GPIOB.split();
    let pins = (
        gpiob.pb13.into_alternate_push_pull(&mut gpiob.crh),
        gpiob.pb14.into_floating_input(&mut gpiob.crh),
        gpiob.pb15.into_alternate_push_pull(&mut gpiob.crh),
    );

    let spi_mode = Mode {
        polarity: Polarity::IdleLow,
        phase: Phase::CaptureOnFirstTransition,
    };
    let spi = Spi::spi2(dp.SPI2, pins, spi_mode, 3.MHz(), clocks);
    // From datasheet:
    //            _______
    // 0 code:   |T0H    |_T0L____|
    //            _________
    // 1 code:   |T1H      |_T1L__|
    //
    // ret:      |____Tret________|
    //
    // 3 MHz gives us 3.3333333333333335e-07 per bit, which is 0.3 us
    // OctoWs2811 has;
    // T0H: 0.3 us
    // T1H: 0.75 us
    // TH_TL: 1.25 us
    // 1.25 / 0.3 = ~ 4.1 may be able to put two bits in a single SPI byte?
    // Start with
    // 0 bit represented by spi byte: 0b10000000
    // 1 bit represented by spi byte: 0b11100000

    // Set up the DMA device
    let dma = dp.DMA1.split();

    // Connect the SPI device to the DMA

    // 
    let buf = singleton!(: [[u8; 8]; 2] = [[0; 8]; 2]).unwrap();

    let spi_dma = spi.with_tx_dma(dma.5);
    // let mut circ_buffer = spi_dma.write(buf);

    // Start a DMA transfer
    let transfer = spi_dma.write(buf);
    // - spi

    
    // Wait for it to finnish. The transfer takes ownership over the SPI device
    // and the data being sent anb those things are returned by transfer.wait
    let (_buffer, _spi_dma) = transfer.wait();



    let mut v = 0usize;
    let mut led_state: bool = false;
    loop {
        v += 1;
        unsafe {
            G_V = v;
            core::ptr::read_volatile(&G_V);
        }
        s.service();
        // wfi();
        if v % 100000 != 0 {
            continue;
        }
        // let z = format!("{}", v);
        let mut d: string::StackString = Default::default();

        if led_state {
            led.set_low();
        } else {
            led.set_high();
        }
        led_state = !led_state;

        core::fmt::write(&mut d, format_args!("{}\n", v)).expect("");
        // v.write_str("\n").unwrap();
        s.write(d.data());
        // delay.delay_ms(1_00_u16);

        while s.available() {
            if let Some(v) = s.read() {
                s.write(&[v - 0x20]);
            } else {
                break;
            }
        }
    }
}
