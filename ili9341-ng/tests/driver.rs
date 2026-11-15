//! Driver tests against the `ili9341-sim` behavioral model.

use core::cell::RefCell;
use fake_bus::FakeDelay;
use ili9341_ng::{ColorDepth, rgb565_to_rgb666, rgb666_to_rgb565};
use ili9341_ng::{
    ColorOrder, Command, Error, GpioBus, HEIGHT, Ili9341, Interface, Orientation, OutputBus,
    ParallelInterface, ReadBus, ReadInterface, SpiInterface, TearingEffect, WIDTH,
};
use ili9341_sim::{FakeSpi, Ili9341Model, InterfaceMode, Line, MODEL_DISPLAY_ID, SimPin};

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
    let m = model.borrow();
    assert_eq!(m.parameters(Command::PWCTRL1.code()), [0x23]);
    assert_eq!(m.parameters(Command::VMCTRL1.code()), [0x3E, 0x28]);
    assert_eq!(m.parameters(Command::PGAMCTRL.code()).len(), 15);
    assert_eq!(m.parameters(Command::NGAMCTRL.code())[14], 0x0F);
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
fn fill_screen_reaches_the_last_pixel() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));
    display.fill_screen(0xFFFF).unwrap();
    let m = model.borrow();
    assert_eq!(m.gram(0, 0), 0xFFFF);
    assert_eq!(m.gram(WIDTH - 1, HEIGHT - 1), 0xFFFF);
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
    // A horizontal run longer than one batch, broken by an off-screen pixel and a jump.
    let run = (0..40)
        .map(|x| Pixel(Point::new(280 + x, 100), Rgb565::new(0, 0, x as u8 % 32)))
        .chain([
            Pixel(Point::new(-1, 100), Rgb565::RED),
            Pixel(Point::new(0, 101), Rgb565::BLUE),
        ]);
    display.draw_iter(run).unwrap();

    let m = model.borrow();
    let red = Rgb565::RED.into_storage();
    assert_eq!((m.gram(3, 0), m.gram(3, 1), m.gram(3, 2)), (red, red, 0));
    assert_eq!((m.gram(238, 10), m.gram(238, 11)), (1, 2));
    assert_eq!((m.gram(239, 10), m.gram(239, 11)), (3, 4));
    assert_eq!(m.gram(5, 5), Rgb565::GREEN.into_storage());
    for x in 0..40u16 {
        let expected = Rgb565::new(0, 0, (x % 32) as u8).into_storage();
        assert_eq!(m.gram(100, 280 + x), expected, "x = {x}");
    }
    assert_eq!(m.gram(101, 0), Rgb565::BLUE.into_storage());
}

/// RGB666 test pixels; three, so the 16-bit bus ends on a padded half word.
const RGB666: [[u8; 3]; 3] = [[63, 0, 1], [1, 2, 3], [32, 33, 34]];

/// Draws RGB666 and RGB565 pixels in 18-bit color and checks the frame memory.
fn check_18bit<DI: Interface>(model: &RefCell<Ili9341Model>, interface: DI)
where
    DI::Error: core::fmt::Debug,
{
    let mut display = init(model, interface);
    display.set_color_depth(ColorDepth::Bits18).unwrap();
    assert_eq!(model.borrow().colmod(), 0x66);
    display.set_orientation(Orientation::Landscape).unwrap();
    display.draw_pixels_rgb666(0, 0, 3, 1, RGB666).unwrap();
    display.draw_pixels(0, 1, 1, 1, [0xF81F]).unwrap();

    let m = model.borrow();
    for (x, pixel) in RGB666.iter().enumerate() {
        assert_eq!(m.gram_rgb666(0, x as u16), *pixel, "x = {x}");
    }
    assert_eq!(m.gram_rgb666(1, 0), [63, 0, 63]);
}

#[test]
fn rgb666_over_spi() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    check_18bit(&model, spi(&model));
}

#[test]
fn rgb666_over_parallel_8bit() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Parallel8));
    let interface = ParallelInterface::new(
        GpioBus::new(SimPin::data8(&model)),
        SimPin::new(&model, Line::Dc),
        SimPin::new(&model, Line::Wr),
    );
    check_18bit(&model, interface);
}

#[test]
fn rgb666_over_parallel_16bit() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Parallel16));
    let interface = ParallelInterface::new(
        GpioBus::new(SimPin::data16(&model)),
        SimPin::new(&model, Line::Dc),
        SimPin::new(&model, Line::Wr),
    );
    check_18bit(&model, interface);
}

#[test]
fn rgb666_in_16bit_color_drops_lsbs() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));
    display
        .draw_pixels_rgb666(0, 0, 1, 1, [[63, 1, 1]])
        .unwrap();
    // Red and blue go through 5 bits and are expanded again by the controller.
    assert_eq!(model.borrow().gram_rgb666(WIDTH - 1, 0), [63, 1, 0]);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "RGB666 components are 0..=63")]
fn rejects_8bit_rgb666_components() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));
    let _ = display.draw_pixels_rgb666(0, 0, 1, 1, [[128, 128, 128]]);
}

#[test]
fn color_conversions() {
    assert_eq!(rgb565_to_rgb666(0xFFFF), [63, 63, 63]);
    assert_eq!(rgb565_to_rgb666(0x0000), [0, 0, 0]);
    assert_eq!(rgb565_to_rgb666(0x8410), [33, 32, 33]);
    for pixel in [0x0000, 0xFFFF, 0x8410, 0x1234, 0xF81F] {
        assert_eq!(rgb666_to_rgb565(rgb565_to_rgb666(pixel)), pixel);
    }
}

#[test]
fn reads_status_and_id() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));

    let mode = display.read_power_mode().unwrap();
    assert!(mode.booster_on() && mode.sleep_out() && mode.normal_mode() && mode.display_on());
    assert!(!mode.partial_mode());
    assert_eq!(
        display.read_memory_access_control().unwrap().into_bits(),
        0x48
    );
    assert_eq!(display.read_pixel_format().unwrap().into_bits(), 0x55);
    assert_eq!(display.read_self_diagnostic().unwrap(), 0xC0);
    assert_eq!(display.read_display_id().unwrap(), MODEL_DISPLAY_ID);
}

#[test]
fn reads_pixels_back() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));
    display
        .draw_pixels(10, 20, 2, 2, [0xF800, 0x07E0, 0x001F, 0x8410])
        .unwrap();

    let mut pixels = [[0; 3]; 4];
    display
        .read_pixels_rgb666(10, 20, 2, 2, &mut pixels)
        .unwrap();
    assert_eq!(
        pixels.map(rgb666_to_rgb565),
        [0xF800, 0x07E0, 0x001F, 0x8410]
    );

    display.set_color_depth(ColorDepth::Bits18).unwrap();
    display.draw_pixels_rgb666(0, 0, 3, 1, RGB666).unwrap();
    let mut pixels = [[0; 3]; 3];
    display.read_pixels_rgb666(0, 0, 3, 1, &mut pixels).unwrap();
    assert_eq!(pixels, RGB666);

    assert_eq!(
        display.read_pixels_rgb666(0, 0, 2, 2, &mut pixels),
        Err(Error::BufferLength)
    );
}

#[test]
fn scrolls() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));

    display.set_scroll_area(10, 20).unwrap();
    assert_eq!(
        model.borrow().parameters(Command::VSCRDEF.code()),
        [0, 10, 1, 34, 0, 20]
    );
    display.set_scroll_start(15).unwrap();
    {
        let m = model.borrow();
        assert!(m.is_scrolling());
        // Fixed areas stay; line 10 shows memory line 15, and the area wraps.
        assert_eq!(m.panel_line(9), Some(9));
        assert_eq!(m.panel_line(10), Some(15));
        assert_eq!(m.panel_line(299), Some(14));
        assert_eq!(m.panel_line(300), Some(300));
    }

    assert_eq!(display.set_scroll_area(300, 21), Err(Error::OutOfBounds));
    // Inside the fixed areas.
    assert_eq!(display.set_scroll_start(9), Err(Error::OutOfBounds));
    assert_eq!(display.set_scroll_start(300), Err(Error::OutOfBounds));
    assert_eq!(display.set_scroll_start(HEIGHT), Err(Error::OutOfBounds));

    display.enter_normal_mode().unwrap();
    assert!(!model.borrow().is_scrolling());
}

#[test]
fn partial_mode() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));

    display.set_partial_area(100, 199).unwrap();
    display.enter_partial_mode().unwrap();
    assert!(display.read_power_mode().unwrap().partial_mode());
    {
        let m = model.borrow();
        assert_eq!(m.panel_line(99), None);
        assert_eq!(m.panel_line(100), Some(100));
        assert_eq!(m.panel_line(199), Some(199));
        assert_eq!(m.panel_line(200), None);
    }

    // End before start wraps around the bottom.
    display.set_partial_area(300, 10).unwrap();
    assert_eq!(model.borrow().panel_line(5), Some(5));
    assert_eq!(model.borrow().panel_line(150), None);
    assert_eq!(display.set_partial_area(0, HEIGHT), Err(Error::OutOfBounds));

    display.enter_normal_mode().unwrap();
    assert!(!model.borrow().is_partial_mode());
    assert_eq!(model.borrow().panel_line(150), Some(150));
}

#[cfg(feature = "async")]
#[test]
fn async_reads() {
    use embassy_futures::block_on;
    use ili9341_ng::asynch::Ili9341;

    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut delay = FakeDelay::new(&model);
    let mut display = Ili9341::new(spi(&model));
    let (id, pixels) = block_on(async {
        display.init(&mut delay).await.unwrap();
        display.fill_rect(0, 0, 2, 1, 0x07E0).await.unwrap();
        display.draw_raw(2, 0, 1, 1, &[0xF8, 0x00]).await.unwrap();
        let id = display.read_display_id().await.unwrap();
        let mut pixels = [[0; 3]; 3];
        display
            .read_pixels_rgb666(0, 0, 3, 1, &mut pixels)
            .await
            .unwrap();
        (id, pixels)
    });
    assert_eq!(id, MODEL_DISPLAY_ID);
    assert_eq!(pixels, [[0, 63, 0], [0, 63, 0], [63, 0, 0]]);
}

#[cfg(feature = "embedded-graphics")]
#[test]
fn embedded_graphics_rgb666() {
    use embedded_graphics::pixelcolor::Rgb666;
    use embedded_graphics::prelude::*;
    use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));
    display.set_color_depth(ColorDepth::Bits18).unwrap();
    display.set_orientation(Orientation::Landscape).unwrap();

    let mut target = display.rgb666();
    assert_eq!(target.bounding_box().size, Size::new(320, 240));
    Rectangle::new(Point::new(318, 0), Size::new(4, 2))
        .into_styled(PrimitiveStyle::with_fill(Rgb666::new(1, 2, 3)))
        .draw(&mut target)
        .unwrap();
    Pixel(Point::new(5, 6), Rgb666::new(63, 31, 7))
        .draw(&mut target)
        .unwrap();

    let m = model.borrow();
    assert_eq!(m.gram_rgb666(1, 319), [1, 2, 3]);
    assert_eq!(m.gram_rgb666(2, 319), [0, 0, 0]);
    assert_eq!(m.gram_rgb666(6, 5), [63, 31, 7]);
}

#[test]
fn draw_raw_writes_slices_in_either_depth() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));
    display.set_orientation(Orientation::Landscape).unwrap();

    display
        .draw_raw(4, 5, 2, 1, &[0xF8, 0x00, 0x00, 0x1F])
        .unwrap();
    assert_eq!(
        display.draw_raw(0, 0, 2, 1, &[0; 3]),
        Err(Error::BufferLength)
    );
    // An empty area needs an empty slice, and sends nothing.
    assert_eq!(display.draw_raw(0, 0, 0, 3, &[]), Ok(()));

    display.set_color_depth(ColorDepth::Bits18).unwrap();
    display
        .draw_raw(4, 6, 1, 1, &[63 << 2, 1 << 2, 2 << 2])
        .unwrap();
    assert_eq!(
        display.draw_raw(0, 0, 1, 1, &[0; 2]),
        Err(Error::BufferLength)
    );

    let m = model.borrow();
    assert_eq!((m.gram(5, 4), m.gram(5, 5)), (0xF800, 0x001F));
    assert_eq!(m.gram_rgb666(6, 4), [63, 1, 2]);
}

#[test]
fn draw_raw_over_parallel() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Parallel16));
    let interface = ParallelInterface::new(
        GpioBus::new(SimPin::data16(&model)),
        SimPin::new(&model, Line::Dc),
        SimPin::new(&model, Line::Wr),
    );
    let mut display = init(&model, interface);
    display.set_orientation(Orientation::Landscape).unwrap();
    display.draw_raw(0, 0, 1, 1, &[0x12, 0x34]).unwrap();
    assert_eq!(model.borrow().gram(0, 0), 0x1234);
}

#[test]
fn fills_with_partial_last_chunk() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));
    display.set_orientation(Orientation::Landscape).unwrap();
    display.fill_screen(0).unwrap();

    // 18-bit: 85 pixels per chunk, so 100 pixels end on a partial chunk.
    display.set_color_depth(ColorDepth::Bits18).unwrap();
    display.fill_rect(0, 0, 100, 1, 0xFFFF).unwrap();
    // 16-bit: 128 pixels per chunk; a full screen is 600 whole chunks.
    display.set_color_depth(ColorDepth::Bits16).unwrap();
    display.fill_rect(0, 1, 129, 1, 0x07E0).unwrap();

    let m = model.borrow();
    assert_eq!(m.gram_rgb666(0, 99), [63, 63, 63]);
    assert_eq!(m.gram_rgb666(0, 100), [0, 0, 0]);
    assert_eq!(m.gram(1, 128), 0x07E0);
    assert_eq!(m.gram(1, 129), 0);
}

#[test]
fn tearing_effect_and_idle_mode() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = init(&model, spi(&model));

    display
        .set_tearing_effect(Some(TearingEffect::VAndHBlank))
        .unwrap();
    assert!(model.borrow().is_tearing_effect_on());
    assert_eq!(model.borrow().parameters(Command::TEON.code()), [1]);
    display
        .set_tearing_effect(Some(TearingEffect::VBlank))
        .unwrap();
    assert_eq!(model.borrow().parameters(Command::TEON.code()), [0]);
    display.set_tearing_effect(None).unwrap();
    assert!(!model.borrow().is_tearing_effect_on());

    display.set_tear_scanline(300).unwrap();
    assert_eq!(
        model.borrow().parameters(Command::SET_TEAR_SCANLINE.code()),
        [1, 44]
    );
    assert_eq!(display.set_tear_scanline(HEIGHT), Err(Error::OutOfBounds));

    // Memory column 0 is the panel's left edge; line 0 its bottom.
    display
        .draw_pixels_rgb666(WIDTH - 1, 0, 1, 1, [[40, 20, 63]])
        .unwrap();
    display.set_idle_mode(true).unwrap();
    assert!(display.read_power_mode().unwrap().idle_mode());
    assert_eq!(model.borrow().panel_pixel(0, HEIGHT - 1), Some([63, 0, 63]));
    display.set_idle_mode(false).unwrap();
    assert!(!model.borrow().is_idle());
}

#[test]
fn init_with_custom_panel_settings() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Spi));
    let mut display = Ili9341::new(spi(&model));
    let panel: &[(u8, &[u8])] = &[(Command::PWCTRL1.code(), &[0x21])];
    display
        .init_with(&mut FakeDelay::new(&model), panel)
        .unwrap();
    let m = model.borrow();
    assert!(m.is_display_on());
    assert_eq!(m.parameters(Command::PWCTRL1.code()), [0x21]);
    assert_eq!(m.parameters(Command::VMCTRL1.code()), [] as [u8; 0]);
    assert_eq!(m.ignored_commands(), 0);
}

/// A parallel bus on [`SimPin`]s whose reads sample what the model drives.
struct SimReadBus<'a, const N: usize> {
    pins: GpioBus<SimPin<'a>, N>,
    model: &'a RefCell<Ili9341Model>,
}

macro_rules! sim_read_bus {
    ($n:literal, $word:ty) => {
        impl OutputBus for SimReadBus<'_, $n> {
            type Word = $word;
            type Error = core::convert::Infallible;

            fn set_value(&mut self, value: $word) -> Result<(), Self::Error> {
                self.pins.set_value(value)
            }
        }

        impl ReadBus for SimReadBus<'_, $n> {
            fn set_input(&mut self) -> Result<(), Self::Error> {
                Ok(())
            }

            fn set_output(&mut self) -> Result<(), Self::Error> {
                Ok(())
            }

            fn read_value(&mut self) -> Result<$word, Self::Error> {
                Ok(self.model.borrow().data_out() as $word)
            }
        }
    };
}

sim_read_bus!(8, u8);
sim_read_bus!(16, u16);

/// Reads status, ID and pixels over a parallel bus.
fn check_parallel_reads<DI>(model: &RefCell<Ili9341Model>, interface: DI)
where
    DI: ReadInterface,
    DI::Error: core::fmt::Debug,
{
    let mut display = init(model, interface);
    let mode = display.read_power_mode().unwrap();
    assert!(mode.sleep_out() && mode.display_on());
    assert_eq!(display.read_pixel_format().unwrap().into_bits(), 0x55);
    assert_eq!(display.read_display_id().unwrap(), MODEL_DISPLAY_ID);

    // Three pixels: nine bytes, so the 16-bit bus ends on a half-used word.
    display.set_color_depth(ColorDepth::Bits18).unwrap();
    display.draw_pixels_rgb666(0, 0, 3, 1, RGB666).unwrap();
    let mut pixels = [[0; 3]; 3];
    display.read_pixels_rgb666(0, 0, 3, 1, &mut pixels).unwrap();
    assert_eq!(pixels, RGB666);
    // Writing still works after a read.
    display.set_pixel(0, 0, 0xFFFF).unwrap();
    assert_eq!(model.borrow().gram(WIDTH - 1, 0), 0xFFFF);
}

#[test]
fn reads_over_parallel_8bit() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Parallel8));
    let bus = SimReadBus {
        pins: GpioBus::new(SimPin::data8(&model)),
        model: &model,
    };
    let interface = ParallelInterface::new(
        bus,
        SimPin::new(&model, Line::Dc),
        SimPin::new(&model, Line::Wr),
    )
    .with_read(SimPin::new(&model, Line::Rd));
    check_parallel_reads(&model, interface);
}

#[test]
fn reads_over_parallel_16bit() {
    let model = RefCell::new(Ili9341Model::new(InterfaceMode::Parallel16));
    let bus = SimReadBus {
        pins: GpioBus::new(SimPin::data16(&model)),
        model: &model,
    };
    let interface = ParallelInterface::new(
        bus,
        SimPin::new(&model, Line::Dc),
        SimPin::new(&model, Line::Wr),
    )
    .with_read(SimPin::new(&model, Line::Rd));
    check_parallel_reads(&model, interface);
}
