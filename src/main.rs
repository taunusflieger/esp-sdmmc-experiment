use embedded_sdmmc::*;
use esp_idf_hal::gpio::*;
use esp_idf_hal::prelude::*;
use esp_idf_hal::spi::config::Duplex;
use esp_idf_hal::spi::*;
use esp_idf_svc::log::EspLogger;
use esp_idf_sys as _;
use log::info;
use log::LevelFilter;
use std::rc::Rc;

static LOGGER: EspLogger = EspLogger;
const FILE_TO_CREATE: &'static str = "GpsLog.txt";

fn main() {
    // Temporary. Will disappear once ESP-IDF 4.4 is released, but for now it is necessary to call this function once,
    // or else some patches to the runtime implemented by esp-idf-sys might not link properly.
    esp_idf_sys::link_patches();

    ::log::set_logger(&LOGGER)
        .map(|()| LOGGER.initialize())
        .unwrap();
    LOGGER.set_target_level("", LevelFilter::Debug);

    info!("SDmmc Test");

    let peripherals = Peripherals::take().unwrap();

    // Note: Rc is not required here, but I tested it here as I require it
    // in the target context
    let driver = std::rc::Rc::new(
        SpiDriver::new(
            peripherals.spi2,
            peripherals.pins.gpio36,       // SCK
            peripherals.pins.gpio35,       // MOSI
            Some(peripherals.pins.gpio37), // MISO
            Dma::Disabled,
        )
        .unwrap(),
    );

    let mut spi_config = SpiConfig::new();
    spi_config.duplex = Duplex::Full;
    let _ = spi_config.baudrate(24.MHz().into());
    let spi = SpiDeviceDriver::new(driver, Option::<Gpio10>::None, &spi_config).unwrap();

    let sdmmc_cs = PinDriver::output(peripherals.pins.gpio10).unwrap();

    let mut sdmmc_spi = embedded_sdmmc::SdMmcSpi::new(spi, sdmmc_cs);
    for _ in 0..9 {
        match sdmmc_spi.acquire() {
            Ok(block) => {
                let mut controller: Controller<
                    BlockSpi<
                        '_,
                        esp_idf_hal::spi::SpiDeviceDriver<'_, Rc<esp_idf_hal::spi::SpiDriver<'_>>>,
                        esp_idf_hal::gpio::PinDriver<'_, Gpio10, esp_idf_hal::gpio::Output>,
                    >,
                    SdMmcClock,
                    5,
                    5,
                > = embedded_sdmmc::Controller::new(block, SdMmcClock);
                info!("OK!");
                info!("Card size...");
                match controller.device().card_size_bytes() {
                    Ok(size) => info!("{}", size),
                    Err(e) => info!("Err: {:?}", e),
                }
                info!("Volume 0...");

                let mut volume = match controller.get_volume(embedded_sdmmc::VolumeIdx(0)) {
                    Ok(v) => v,
                    Err(e) => panic!("Err: {:?}", e),
                };

                let root_dir = match controller.open_root_dir(&volume) {
                    Ok(d) => d,
                    Err(e) => panic!("Err: {:?}", e),
                };

                info!("creating file {}", FILE_TO_CREATE);
                let mut f = match controller.open_file_in_dir(
                    &mut volume,
                    &root_dir,
                    FILE_TO_CREATE,
                    Mode::ReadWriteCreateOrAppend,
                ) {
                    Ok(f) => f,
                    Err(e) => panic!("Err: {:?}", e),
                };

                f.seek_from_end(0).unwrap();
                let buffer1 = b"0123456789\n";
                let num_written = match controller.write(&mut volume, &mut f, &buffer1[..]) {
                    Ok(num) => num,
                    Err(e) => panic!("Err: {:?}", e),
                };
                info!("Bytes written {}", num_written);
                match controller.close_file(&volume, f) {
                    Ok(_) => info!("file closed"),
                    Err(e) => panic!("Err: {:?}", e),
                };
            }
            Err(e) => info!("Error acquire SPI bus {:?}", e),
        }
        esp_idf_hal::delay::FreeRtos::delay_ms(50);
    }
    loop {
        esp_idf_hal::delay::FreeRtos::delay_ms(100);
    }
}

pub struct SdMmcClock;

impl TimeSource for SdMmcClock {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}
