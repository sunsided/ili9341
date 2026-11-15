//! Driver tests against the `ili9341-sim` behavioral model.

use core::cell::RefCell;
use fake_bus::FakeDelay;
use ili9341_ng::{
    ColorOrder, Error, GpioBus, HEIGHT, Ili9341, Interface, Orientation, ParallelInterface,
    SpiInterface, WIDTH,
};
use ili9341_sim::{FakeSpi, Ili9341Model, InterfaceMode, Line, SimPin};

type Spi<'a> = SpiInterface<FakeSpi<'a>, SimPin<'a>>;

fn spi(model: &RefCell<Ili9341Model>) -> Spi<'_> {
    SpiInterface::new(FakeSpi::new(model), SimPin::new(model, Line::Dc))
}

/// Initializes a driver and checks that the model accepted every command.
fn init<DI: Interface>(model: &RefCell<Ili9341Model>, interface: DI) -> Ili9341<DI>
where
    DI::Error: core::fmt::Debug,
{
    let mut display = Ili9341::new(interface);
    display.init(&mut FakeDelay::new(model)).unwrap();
    let m = model.borrow();
    assert!(!m.is_sleeping());
    assert!(m.is_display_on());
    assert_eq!(m.colmod(), 0x55);
    assert_eq!(m.ignored_commands(), 0);
    display
}

#[test]
fn init_configures_controller() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let display = init(&model, spi(&model));
    assert_eq!(model.borrow().madctl().into_bits(), 0x48); // MX | BGR
    assert_eq!(display.dimensions(), (WIDTH, HEIGHT));
}

#[test]
fn hardware_reset_then_init() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut delay = FakeDelay::new(&model);
    ili9341_ng::reset(&mut SimPin::new(&model, Line::Reset), &mut delay).unwrap();
    init(&model, spi(&model));
}

#[test]
fn fill_rect_writes_only_the_area() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));
    display.set_orientation(Orientation::Landscape).unwrap(); // no mirroring
    display.fill_screen(0x0000).unwrap();
    display.fill_rect(10, 20, 3, 2, 0xF800).unwrap();

    let m = model.borrow();
    // Landscape exchanges rows and columns: memory column = y, row = x.
    assert_eq!(m.gram(20, 10), 0xF800);
    assert_eq!(m.gram(21, 12), 0xF800);
    assert_eq!(m.gram(22, 12), 0x0000);
    assert_eq!(m.gram(21, 13), 0x0000);
}

#[test]
fn orientations_rotate_the_origin() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));
    let (w, h) = (WIDTH - 1, HEIGHT - 1);

    // Memory corner that logical (0, 0) lands in; the common module glass mirrors
    // memory columns, so these are the visible top-left corners of each rotation.
    for (orientation, corner, size) in [
        (Orientation::Portrait, (w, 0), (WIDTH, HEIGHT)),
        (Orientation::Landscape, (0, 0), (HEIGHT, WIDTH)),
        (Orientation::PortraitFlipped, (0, h), (WIDTH, HEIGHT)),
        (Orientation::LandscapeFlipped, (w, h), (HEIGHT, WIDTH)),
    ] {
        display.set_orientation(orientation).unwrap();
        assert_eq!(display.dimensions(), size);
        display.set_pixel(0, 0, 0x1234).unwrap();
        assert_eq!(
            model.borrow().gram(corner.0, corner.1),
            0x1234,
            "{orientation:?}"
        );
        display.set_pixel(0, 0, 0).unwrap();
    }
}

#[test]
fn draw_pixels_streams_row_by_row() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));
    display
        .set_orientation(Orientation::PortraitFlipped)
        .unwrap(); // only MY
    // More pixels than one SPI chunk, to cover chunking.
    display.draw_pixels(0, 0, 50, 2, 0..100).unwrap();

    let m = model.borrow();
    assert_eq!(m.gram(0, HEIGHT - 1), 0);
    assert_eq!(m.gram(49, HEIGHT - 1), 49);
    assert_eq!(m.gram(0, HEIGHT - 2), 50);
    assert_eq!(m.gram(49, HEIGHT - 2), 99);
}

#[test]
fn rejects_areas_outside_the_screen() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));
    assert_eq!(
        display.fill_rect(WIDTH - 1, 0, 2, 1, 0),
        Err(Error::OutOfBounds)
    );
    assert_eq!(
        display.fill_rect(0, HEIGHT, 1, 1, 0),
        Err(Error::OutOfBounds)
    );
    assert_eq!(
        display.fill_rect(u16::MAX, 0, u16::MAX, 1, 0),
        Err(Error::OutOfBounds)
    );
    assert_eq!(display.fill_rect(0, 0, 0, 5, 0), Ok(()));
}

#[test]
fn display_state_commands() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));
    let mut delay = FakeDelay::new(&model);

    display.set_inverted(true).unwrap();
    display.set_color_order(ColorOrder::Rgb).unwrap();
    display.set_display_on(false).unwrap();
    {
        let m = model.borrow();
        assert!(m.is_inverted());
        assert!(!m.madctl().bgr());
        assert!(!m.is_display_on());
    }

    display.sleep(&mut delay).unwrap();
    assert!(model.borrow().is_sleeping());
    display.wake(&mut delay).unwrap();
    assert!(!model.borrow().is_sleeping());
    assert_eq!(model.borrow().ignored_commands(), 0);
}

/// Draws the same pattern over a parallel bus and checks the frame memory.
fn check_parallel<DI: Interface>(model: &RefCell<Ili9341Model>, interface: DI)
where
    DI::Error: core::fmt::Debug,
{
    let mut display = init(model, interface);
    display.set_orientation(Orientation::Landscape).unwrap();
    display.draw_pixels(5, 7, 2, 1, [0xABCD, 0x00FF]).unwrap();
    let m = model.borrow();
    assert_eq!(m.gram(7, 5), 0xABCD);
    assert_eq!(m.gram(7, 6), 0x00FF);
}

#[test]
fn parallel_8bit() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Parallel8));
    let interface = ParallelInterface::new(
        GpioBus::new(SimPin::data8(&model)),
        SimPin::new(&model, Line::Dc),
        SimPin::new(&model, Line::Wr),
    );
    check_parallel(&model, interface);
}

#[test]
fn parallel_16bit() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Parallel16));
    let interface = ParallelInterface::new(
        GpioBus::new(SimPin::data16(&model)),
        SimPin::new(&model, Line::Dc),
        SimPin::new(&model, Line::Wr),
    );
    check_parallel(&model, interface);
}

#[cfg(feature = "async")]
#[test]
fn async_spi() {
    use embassy_futures::block_on;
    use ili9341_ng::asynch::Ili9341;

    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut delay = FakeDelay::new(&model);
    let mut display = Ili9341::new(spi(&model));
    block_on(async {
        ili9341_ng::asynch::reset(&mut SimPin::new(&model, Line::Reset), &mut delay).await?;
        display.init(&mut delay).await.unwrap();
        display
            .set_orientation(Orientation::Landscape)
            .await
            .unwrap();
        display.fill_rect(1, 2, 1, 1, 0x07E0).await.unwrap();
        Ok::<_, core::convert::Infallible>(())
    })
    .unwrap();

    let m = model.borrow();
    assert!(m.is_display_on());
    assert_eq!(m.ignored_commands(), 0);
    assert_eq!(m.gram(2, 1), 0x07E0);
}

#[cfg(feature = "embedded-graphics")]
#[test]
fn embedded_graphics_clips() {
    use embedded_graphics::pixelcolor::Rgb565;
    use embedded_graphics::prelude::*;
    use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));
    display.set_orientation(Orientation::Landscape).unwrap();
    display.clear(Rgb565::BLACK).unwrap();
    assert_eq!(display.bounding_box().size, Size::new(320, 240));

    // Half off the left edge: only x = 0..2 is drawn.
    Rectangle::new(Point::new(-2, 3), Size::new(4, 1))
        .into_styled(PrimitiveStyle::with_fill(Rgb565::RED))
        .draw(&mut display)
        .unwrap();
    // Contiguous fill, partly off the bottom: rows 238 and 239 only.
    display
        .fill_contiguous(
            &Rectangle::new(Point::new(10, 238), Size::new(2, 3)),
            [1, 2, 3, 4, 5, 6]
                .map(|v| Rgb565::from(embedded_graphics::pixelcolor::raw::RawU16::new(v))),
        )
        .unwrap();
    Pixel(Point::new(400, 5), Rgb565::GREEN)
        .draw(&mut display)
        .unwrap();
    Pixel(Point::new(5, 5), Rgb565::GREEN)
        .draw(&mut display)
        .unwrap();

    let m = model.borrow();
    let red = Rgb565::RED.into_storage();
    assert_eq!((m.gram(3, 0), m.gram(3, 1), m.gram(3, 2)), (red, red, 0));
    assert_eq!((m.gram(238, 10), m.gram(238, 11)), (1, 2));
    assert_eq!((m.gram(239, 10), m.gram(239, 11)), (3, 4));
    assert_eq!(m.gram(5, 5), Rgb565::GREEN.into_storage());
}
