//! The driver, written once for both the blocking and the async API.
//!
//! [`driver!`] expands to [`reset`](crate::reset) and [`Ili9341`](crate::Ili9341) in the
//! crate root, and to their async counterparts in `asynch`. The async expansion only
//! differs in the `async` keyword on functions and `.await` after interface and delay
//! calls.

/// Generates `reset`, the `Ili9341` driver struct and its methods.
///
/// - `interface` / `read_interface` / `delay`: the interface and delay traits to build on.
/// - `async`: empty, or `async` to make the methods async.
/// - `await`: empty, or `.await` to await interface and delay calls.
macro_rules! driver {
    (
        doc = $doc:literal,
        interface = $interface:ident,
        read_interface = $read_interface:ident,
        delay = $delay:path,
        async = [$($asy:tt)*],
        await = [$($aw:tt)*] $(,)?
    ) => {
        /// Resets the controller via its active-low `RESX` pin and waits until it accepts
        /// commands.
        pub $($asy)* fn reset<P, D>(rst: &mut P, delay: &mut D) -> Result<(), P::Error>
        where
            P: embedded_hal::digital::OutputPin,
            D: $delay,
        {
            rst.set_low()?;
            delay.delay_us($crate::RESET_PULSE_US) $($aw)*;
            rst.set_high()?;
            delay.delay_ms($crate::RESET_DELAY_MS) $($aw)*;
            Ok(())
        }

        #[doc = $doc]
        #[derive(Debug)]
        pub struct Ili9341<DI> {
            interface: DI,
            madctl: $crate::registers::MemoryAccessControl,
            orientation: $crate::Orientation,
            color_depth: $crate::ColorDepth,
            /// The first scrolling line and the number of scrolling lines (`VSCRDEF`).
            scroll_area: (u16, u16),
        }

        impl<DI> Ili9341<DI> {
            /// Creates a driver in [`Portrait`](crate::Orientation::Portrait) orientation
            /// with [`Bgr`](crate::ColorOrder::Bgr) color order and
            /// [`Bits16`](crate::ColorDepth::Bits16) color. Performs no bus access; call
            /// [`init`](Self::init) next.
            pub fn new(interface: DI) -> Self {
                let orientation = $crate::Orientation::Portrait;
                let madctl = orientation.apply($crate::registers::MemoryAccessControl::new().with_bgr(true));
                Self {
                    interface,
                    madctl,
                    orientation,
                    color_depth: $crate::ColorDepth::Bits16,
                    scroll_area: (0, $crate::HEIGHT),
                }
            }

            /// Consumes the driver and returns the interface.
            pub fn release(self) -> DI {
                self.interface
            }

            /// The current orientation.
            pub fn orientation(&self) -> $crate::Orientation {
                self.orientation
            }

            /// The width and height in the current orientation.
            pub fn dimensions(&self) -> (u16, u16) {
                self.orientation.size()
            }

            /// The current color depth.
            pub fn color_depth(&self) -> $crate::ColorDepth {
                self.color_depth
            }

            /// Checks that `width × height` pixels of `unit` elements each fill a buffer
            /// of `len` elements.
            fn check_buffer<E>(width: u16, height: u16, unit: usize, len: usize) -> Result<(), $crate::Error<E>> {
                // u32: the product overflows a 16-bit usize for a full screen.
                let expected = u32::from(width) * u32::from(height) * unit as u32;
                match u32::try_from(len) {
                    Ok(len) if len == expected => Ok(()),
                    _ => Err($crate::Error::BufferLength),
                }
            }

            /// Checks that the area lies within the screen and encodes its column and page
            /// address ranges. Returns `None` for an empty area.
            #[allow(clippy::type_complexity)]
            fn window<E>(
                &self,
                x: u16,
                y: u16,
                width: u16,
                height: u16,
            ) -> Result<Option<([u8; 4], [u8; 4])>, $crate::Error<E>> {
                if width == 0 || height == 0 {
                    return Ok(None);
                }
                let (screen_width, screen_height) = self.dimensions();
                if u32::from(x) + u32::from(width) > u32::from(screen_width)
                    || u32::from(y) + u32::from(height) > u32::from(screen_height)
                {
                    return Err($crate::Error::OutOfBounds);
                }
                Ok(Some((
                    $crate::registers::address_range(x, x + width - 1),
                    $crate::registers::address_range(y, y + height - 1),
                )))
            }
        }

        impl<DI> Ili9341<DI>
        where
            DI: $interface,
        {
            /// Initializes the controller: software reset, the Adafruit power, VCOM, frame
            /// rate and gamma settings ([`PANEL_INIT`](crate::PANEL_INIT)), the stored
            /// color depth, orientation and color order, sleep out and display on.
            ///
            /// Takes about 240 ms. The frame memory content is undefined afterwards.
            pub $($asy)* fn init<D>(&mut self, delay: &mut D) -> Result<(), $crate::Error<DI::Error>>
            where
                D: $delay,
            {
                self.init_with(delay, $crate::PANEL_INIT) $($aw)*
            }

            /// Initializes the controller like [`init`](Self::init), with your own panel
            /// settings: `panel` is a list of `(command, parameters)`, sent after the
            /// software reset in place of [`PANEL_INIT`](crate::PANEL_INIT).
            pub $($asy)* fn init_with<D>(
                &mut self,
                delay: &mut D,
                panel: &[(u8, &[u8])],
            ) -> Result<(), $crate::Error<DI::Error>>
            where
                D: $delay,
            {
                use $crate::Command;

                self.send_command(Command::SWRESET, &[]) $($aw)* ?;
                self.scroll_area = (0, $crate::HEIGHT);
                delay.delay_ms($crate::RESET_DELAY_MS) $($aw)*;
                for &(command, parameters) in panel {
                    self.interface
                        .send_command(command, parameters)
                        $($aw)*
                        .map_err($crate::Error::Interface)?;
                }
                self.send_command(Command::COLMOD, &[self.color_depth.pixel_format().into_bits()]) $($aw)* ?;
                self.send_command(Command::MADCTL, &[self.madctl.into_bits()]) $($aw)* ?;
                self.send_command(Command::SLPOUT, &[]) $($aw)* ?;
                delay.delay_ms($crate::SLEEP_OUT_DELAY_MS) $($aw)*;
                self.send_command(Command::DISPON, &[]) $($aw)*
            }

            /// Sends a command and its parameters.
            pub $($asy)* fn send_command(
                &mut self,
                command: $crate::Command,
                parameters: &[u8],
            ) -> Result<(), $crate::Error<DI::Error>> {
                self.interface
                    .send_command(command.code(), parameters)
                    $($aw)*
                    .map_err($crate::Error::Interface)
            }

            /// Sets the orientation. The frame memory content is kept, but appears rotated.
            pub $($asy)* fn set_orientation(
                &mut self,
                orientation: $crate::Orientation,
            ) -> Result<(), $crate::Error<DI::Error>> {
                self.orientation = orientation;
                self.madctl = orientation.apply(self.madctl);
                self.send_command($crate::Command::MADCTL, &[self.madctl.into_bits()]) $($aw)*
            }

            /// Sets the color filter order of the panel.
            pub $($asy)* fn set_color_order(
                &mut self,
                order: $crate::ColorOrder,
            ) -> Result<(), $crate::Error<DI::Error>> {
                self.madctl = self.madctl.with_bgr(order == $crate::ColorOrder::Bgr);
                self.send_command($crate::Command::MADCTL, &[self.madctl.into_bits()]) $($aw)*
            }

            /// Sets the color depth of the pixel interface. The frame memory content is
            /// kept.
            pub $($asy)* fn set_color_depth(
                &mut self,
                depth: $crate::ColorDepth,
            ) -> Result<(), $crate::Error<DI::Error>> {
                self.color_depth = depth;
                self.send_command($crate::Command::COLMOD, &[depth.pixel_format().into_bits()]) $($aw)*
            }

            /// Inverts the displayed colors. Some panels need this to show correct colors.
            pub $($asy)* fn set_inverted(&mut self, inverted: bool) -> Result<(), $crate::Error<DI::Error>> {
                let command = if inverted { $crate::Command::INVON } else { $crate::Command::INVOFF };
                self.send_command(command, &[]) $($aw)*
            }

            /// Turns the display output on or off. The frame memory is kept.
            pub $($asy)* fn set_display_on(&mut self, on: bool) -> Result<(), $crate::Error<DI::Error>> {
                let command = if on { $crate::Command::DISPON } else { $crate::Command::DISPOFF };
                self.send_command(command, &[]) $($aw)*
            }

            /// Turns idle mode on or off. In idle mode, the panel shows 8 colors: the most
            /// significant bit of each component.
            pub $($asy)* fn set_idle_mode(&mut self, idle: bool) -> Result<(), $crate::Error<DI::Error>> {
                let command = if idle { $crate::Command::IDMON } else { $crate::Command::IDMOFF };
                self.send_command(command, &[]) $($aw)*
            }

            /// Turns the tearing effect output `TE` on with `Some` mode, or off with `None`.
            pub $($asy)* fn set_tearing_effect(
                &mut self,
                mode: Option<$crate::TearingEffect>,
            ) -> Result<(), $crate::Error<DI::Error>> {
                match mode {
                    None => self.send_command($crate::Command::TEOFF, &[]) $($aw)*,
                    Some(mode) => {
                        let both = u8::from(mode == $crate::TearingEffect::VAndHBlank);
                        self.send_command($crate::Command::TEON, &[both]) $($aw)*
                    }
                }
            }

            /// Sets the panel line (`0..320`) at which `TE` goes high in
            /// [`VBlank`](crate::TearingEffect::VBlank) mode; `0` is the start of the
            /// vertical blanking.
            pub $($asy)* fn set_tear_scanline(&mut self, line: u16) -> Result<(), $crate::Error<DI::Error>> {
                if line >= $crate::HEIGHT {
                    return Err($crate::Error::OutOfBounds);
                }
                self.send_command($crate::Command::SET_TEAR_SCANLINE, &line.to_be_bytes()) $($aw)*
            }

            /// Sets the frame rate in normal mode (`FRMCTR1`): `division` (`0..=3`) divides
            /// the 615 kHz oscillator by `2^division`, and each line takes
            /// `clocks_per_line` (`16..=31`) of its cycles.
            ///
            /// Frame rate = 615 kHz / (2^`division` × `clocks_per_line` × 324 lines); with
            /// `division` 0, from 119 Hz (16 clocks) to 61 Hz (31). [`init`](Self::init)
            /// sets 79 Hz (24), the controller resets to 70 Hz (27).
            pub $($asy)* fn set_frame_rate(
                &mut self,
                division: u8,
                clocks_per_line: u8,
            ) -> Result<(), $crate::Error<DI::Error>> {
                if division > 3 || !(16..=31).contains(&clocks_per_line) {
                    return Err($crate::Error::OutOfBounds);
                }
                self.send_command($crate::Command::FRMCTR1, &[division, clocks_per_line]) $($aw)*
            }

            /// Enters sleep mode. The frame memory is kept.
            pub $($asy)* fn sleep<D>(&mut self, delay: &mut D) -> Result<(), $crate::Error<DI::Error>>
            where
                D: $delay,
            {
                self.send_command($crate::Command::SLPIN, &[]) $($aw)* ?;
                delay.delay_ms($crate::SLEEP_IN_DELAY_MS) $($aw)*;
                Ok(())
            }

            /// Leaves sleep mode.
            pub $($asy)* fn wake<D>(&mut self, delay: &mut D) -> Result<(), $crate::Error<DI::Error>>
            where
                D: $delay,
            {
                self.send_command($crate::Command::SLPOUT, &[]) $($aw)* ?;
                delay.delay_ms($crate::SLEEP_OUT_DELAY_MS) $($aw)*;
                Ok(())
            }

            /// Sets the column and page address window. Returns `false` for an empty area.
            $($asy)* fn set_window(
                &mut self,
                x: u16,
                y: u16,
                width: u16,
                height: u16,
            ) -> Result<bool, $crate::Error<DI::Error>> {
                let Some((columns, pages)) = self.window(x, y, width, height)? else {
                    return Ok(false);
                };
                self.send_command($crate::Command::CASET, &columns) $($aw)* ?;
                self.send_command($crate::Command::PASET, &pages) $($aw)* ?;
                Ok(true)
            }

            /// Writes pixels of either format into an area, converted to the color depth.
            #[allow(private_bounds)]
            $($asy)* fn write_pixels<P, I>(
                &mut self,
                x: u16,
                y: u16,
                width: u16,
                height: u16,
                pixels: I,
            ) -> Result<(), $crate::Error<DI::Error>>
            where
                P: $crate::PixelData,
                I: IntoIterator<Item = P>,
            {
                if !self.set_window(x, y, width, height) $($aw)* ? {
                    return Ok(());
                }
                self.send_command($crate::Command::RAMWR, &[]) $($aw)* ?;
                let bytes = $crate::pixel_bytes(pixels, self.color_depth);
                self.interface.send_pixel_data(bytes) $($aw)* .map_err($crate::Error::Interface)
            }

            /// Writes RGB565 pixels into an area, row by row.
            ///
            /// Supply `width × height` pixels. Fewer leave the rest of the area unchanged;
            /// more wrap around to the start of the area.
            pub $($asy)* fn draw_pixels<I>(
                &mut self,
                x: u16,
                y: u16,
                width: u16,
                height: u16,
                pixels: I,
            ) -> Result<(), $crate::Error<DI::Error>>
            where
                I: IntoIterator<Item = u16>,
            {
                self.write_pixels(x, y, width, height, pixels) $($aw)*
            }

            /// Writes RGB666 pixels (`[r, g, b]`, components `0..=63`) into an area, row
            /// by row, like [`draw_pixels`](Self::draw_pixels). Debug builds panic on
            /// larger components. In 16-bit color depth, red and
            /// blue lose their least significant bit.
            pub $($asy)* fn draw_pixels_rgb666<I>(
                &mut self,
                x: u16,
                y: u16,
                width: u16,
                height: u16,
                pixels: I,
            ) -> Result<(), $crate::Error<DI::Error>>
            where
                I: IntoIterator<Item = [u8; 3]>,
            {
                self.write_pixels(x, y, width, height, pixels) $($aw)*
            }

            /// Writes pixel data from a slice into an area, row by row, in the current
            /// color depth: two bytes per pixel (RGB565, big-endian) in 16-bit color, three
            /// (`[r, g, b]`, components in the upper six bits) in 18-bit color.
            ///
            /// `data` must hold exactly `width × height` pixels. Over SPI, it goes to the
            /// device in one write, which a DMA-capable device can send without copying; use
            /// it to flush a framebuffer, such as the big-endian `Framebuffer` of
            /// `embedded-graphics`.
            pub $($asy)* fn draw_raw(
                &mut self,
                x: u16,
                y: u16,
                width: u16,
                height: u16,
                data: &[u8],
            ) -> Result<(), $crate::Error<DI::Error>> {
                Self::check_buffer(width, height, self.color_depth.bytes_per_pixel(), data.len())?;
                if !self.set_window(x, y, width, height) $($aw)* ? {
                    return Ok(());
                }
                self.send_command($crate::Command::RAMWR, &[]) $($aw)* ?;
                self.interface.send_pixel_slice(data) $($aw)* .map_err($crate::Error::Interface)
            }

            /// Fills an area with one color.
            pub $($asy)* fn fill_rect(
                &mut self,
                x: u16,
                y: u16,
                width: u16,
                height: u16,
                color: u16,
            ) -> Result<(), $crate::Error<DI::Error>> {
                self.fill(x, y, width, height, color) $($aw)*
            }

            /// Fills an area with one color of either pixel format.
            #[allow(private_bounds)]
            $($asy)* fn fill<P: $crate::PixelData>(
                &mut self,
                x: u16,
                y: u16,
                width: u16,
                height: u16,
                color: P,
            ) -> Result<(), $crate::Error<DI::Error>> {
                if !self.set_window(x, y, width, height) $($aw)* ? {
                    return Ok(());
                }
                self.send_command($crate::Command::RAMWR, &[]) $($aw)* ?;
                let (bytes, len) = color.encode(self.color_depth);
                let count = u32::from(width) * u32::from(height);
                self.interface
                    .send_pixel_repeat(&bytes[..len], count)
                    $($aw)*
                    .map_err($crate::Error::Interface)
            }

            /// Fills the whole screen with one color.
            pub $($asy)* fn fill_screen(&mut self, color: u16) -> Result<(), $crate::Error<DI::Error>> {
                let (width, height) = self.dimensions();
                self.fill_rect(0, 0, width, height, color) $($aw)*
            }

            /// Sets a single pixel.
            pub $($asy)* fn set_pixel(&mut self, x: u16, y: u16, color: u16) -> Result<(), $crate::Error<DI::Error>> {
                self.draw_pixels(x, y, 1, 1, [color]) $($aw)*
            }

            /// Defines the vertical scrolling area: the frame memory lines between
            /// `top_fixed` lines at the top and `bottom_fixed` lines at the bottom scroll;
            /// the fixed lines stay in place.
            ///
            /// Lines run along the 320-pixel side of the panel, whatever the orientation.
            /// Scrolling starts with [`set_scroll_start`](Self::set_scroll_start).
            pub $($asy)* fn set_scroll_area(
                &mut self,
                top_fixed: u16,
                bottom_fixed: u16,
            ) -> Result<(), $crate::Error<DI::Error>> {
                let Some(scrolling) = $crate::HEIGHT
                    .checked_sub(top_fixed)
                    .and_then(|rest| rest.checked_sub(bottom_fixed))
                else {
                    return Err($crate::Error::OutOfBounds);
                };
                let [t0, t1] = top_fixed.to_be_bytes();
                let [s0, s1] = scrolling.to_be_bytes();
                let [b0, b1] = bottom_fixed.to_be_bytes();
                self.send_command($crate::Command::VSCRDEF, &[t0, t1, s0, s1, b0, b1]) $($aw)* ?;
                self.scroll_area = (top_fixed, scrolling);
                Ok(())
            }

            /// Enters vertical scroll mode: `line` is the frame memory line shown right
            /// below the top fixed area. It must lie within the scrolling area set by
            /// [`set_scroll_area`](Self::set_scroll_area), or the whole screen without one.
            ///
            /// The controller ignores this in partial mode.
            /// [`enter_normal_mode`](Self::enter_normal_mode) leaves scroll mode.
            pub $($asy)* fn set_scroll_start(&mut self, line: u16) -> Result<(), $crate::Error<DI::Error>> {
                let (top, scrolling) = self.scroll_area;
                if !(top..top + scrolling).contains(&line) {
                    return Err($crate::Error::OutOfBounds);
                }
                self.send_command($crate::Command::VSCRSADD, &line.to_be_bytes()) $($aw)*
            }

            /// Defines the partial area: frame memory lines `start` to `end`, inclusive.
            /// With `end < start`, the area wraps around the bottom.
            pub $($asy)* fn set_partial_area(
                &mut self,
                start: u16,
                end: u16,
            ) -> Result<(), $crate::Error<DI::Error>> {
                if start >= $crate::HEIGHT || end >= $crate::HEIGHT {
                    return Err($crate::Error::OutOfBounds);
                }
                let range = $crate::registers::address_range(start, end);
                self.send_command($crate::Command::PTLAR, &range) $($aw)*
            }

            /// Enters partial mode: only the partial area is shown. Leaves scroll mode.
            pub $($asy)* fn enter_partial_mode(&mut self) -> Result<(), $crate::Error<DI::Error>> {
                self.send_command($crate::Command::PTLON, &[]) $($aw)*
            }

            /// Enters normal display mode: leaves partial mode and scroll mode.
            pub $($asy)* fn enter_normal_mode(&mut self) -> Result<(), $crate::Error<DI::Error>> {
                self.send_command($crate::Command::NORON, &[]) $($aw)*
            }
        }

        impl<DI> Ili9341<DI>
        where
            DI: $read_interface,
        {
            /// Sends a read command and reads its response.
            $($asy)* fn read(
                &mut self,
                command: $crate::Command,
                buffer: &mut [u8],
            ) -> Result<(), $crate::Error<DI::Error>> {
                self.interface
                    .read(command.code(), buffer)
                    $($aw)*
                    .map_err($crate::Error::Interface)
            }

            /// Reads one byte from an 8-bit read command.
            $($asy)* fn read_u8(&mut self, command: $crate::Command) -> Result<u8, $crate::Error<DI::Error>> {
                let mut buffer = [0];
                self.read(command, &mut buffer) $($aw)* ?;
                Ok(buffer[0])
            }

            /// Reads the display power mode (`RDDPM`).
            pub $($asy)* fn read_power_mode(&mut self) -> Result<$crate::registers::PowerMode, $crate::Error<DI::Error>> {
                Ok($crate::registers::PowerMode::from_bits(self.read_u8($crate::Command::RDDPM) $($aw)* ?))
            }

            /// Reads the memory access control (`RDDMADCTL`).
            pub $($asy)* fn read_memory_access_control(
                &mut self,
            ) -> Result<$crate::registers::MemoryAccessControl, $crate::Error<DI::Error>> {
                Ok($crate::registers::MemoryAccessControl::from_bits(self.read_u8($crate::Command::RDDMADCTL) $($aw)* ?))
            }

            /// Reads the pixel format (`RDDCOLMOD`).
            pub $($asy)* fn read_pixel_format(&mut self) -> Result<$crate::registers::PixelFormat, $crate::Error<DI::Error>> {
                Ok($crate::registers::PixelFormat::from_bits(self.read_u8($crate::Command::RDDCOLMOD) $($aw)* ?))
            }

            /// Reads the self-diagnostic result (`RDDSDR`): bit 7 reports that the
            /// register values loaded correctly, bit 6 that the display works.
            pub $($asy)* fn read_self_diagnostic(&mut self) -> Result<u8, $crate::Error<DI::Error>> {
                self.read_u8($crate::Command::RDDSDR) $($aw)*
            }

            /// Reads the display status (`RDDST`), 32 bits with the datasheet's layout:
            /// booster, `MADCTL`, pixel format, idle, partial, sleep, normal and scroll
            /// mode, inversion, display on, and the tearing effect.
            pub $($asy)* fn read_display_status(&mut self) -> Result<u32, $crate::Error<DI::Error>> {
                let mut buffer = [0; 4];
                self.read($crate::Command::RDDST, &mut buffer) $($aw)* ?;
                Ok(u32::from_be_bytes(buffer))
            }

            /// Reads the display image mode (`RDDIM`): bit 5 is inversion, bits 2–0 the
            /// gamma curve.
            pub $($asy)* fn read_image_mode(&mut self) -> Result<u8, $crate::Error<DI::Error>> {
                self.read_u8($crate::Command::RDDIM) $($aw)*
            }

            /// Reads the display signal mode (`RDDSM`): bit 7 is the tearing effect output,
            /// bit 6 its mode (set for [`VAndHBlank`](crate::TearingEffect::VAndHBlank)).
            pub $($asy)* fn read_signal_mode(&mut self) -> Result<u8, $crate::Error<DI::Error>> {
                self.read_u8($crate::Command::RDDSM) $($aw)*
            }

            /// Reads the scanline the panel refresh is at (`GET_SCANLINE`), to time frame
            /// memory writes without a `TE` line.
            pub $($asy)* fn read_scanline(&mut self) -> Result<u16, $crate::Error<DI::Error>> {
                let mut buffer = [0; 2];
                self.read($crate::Command::GET_SCANLINE, &mut buffer) $($aw)* ?;
                Ok(u16::from_be_bytes(buffer) & 0x3FF)
            }

            /// Reads the manufacturer, version and driver ID with three 8-bit reads
            /// (`RDID1`–`RDID3`); the same values as
            /// [`read_display_id`](Self::read_display_id), without its dummy clock.
            pub $($asy)* fn read_ids(&mut self) -> Result<[u8; 3], $crate::Error<DI::Error>> {
                Ok([
                    self.read_u8($crate::Command::RDID1) $($aw)* ?,
                    self.read_u8($crate::Command::RDID2) $($aw)* ?,
                    self.read_u8($crate::Command::RDID3) $($aw)* ?,
                ])
            }

            /// Reads the display identification (`RDDIDIF`): manufacturer ID, module or
            /// driver version, and module or driver ID. The values depend on the module.
            pub $($asy)* fn read_display_id(&mut self) -> Result<[u8; 3], $crate::Error<DI::Error>> {
                let mut buffer = [0; 3];
                self.read($crate::Command::RDDIDIF, &mut buffer) $($aw)* ?;
                Ok(buffer)
            }

            /// Reads pixels from an area of the frame memory, row by row, as RGB666
            /// (`[r, g, b]`, 6-bit components) whatever the color depth.
            ///
            /// `pixels` must hold exactly `width × height` pixels. Pixels written in
            /// 16-bit color have red and blue expanded by the controller.
            pub $($asy)* fn read_pixels_rgb666(
                &mut self,
                x: u16,
                y: u16,
                width: u16,
                height: u16,
                pixels: &mut [[u8; 3]],
            ) -> Result<(), $crate::Error<DI::Error>> {
                Self::check_buffer(width, height, 1, pixels.len())?;
                if !self.set_window(x, y, width, height) $($aw)* ? {
                    return Ok(());
                }
                // Components arrive in the upper six bits.
                let bytes = pixels.as_flattened_mut();
                self.read($crate::Command::RAMRD, bytes) $($aw)* ?;
                bytes.iter_mut().for_each(|byte| *byte >>= 2);
                Ok(())
            }
        }
    };
}
