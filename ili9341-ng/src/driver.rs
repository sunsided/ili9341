//! The driver, written once for both the blocking and the async API.
//!
//! [`driver!`] expands to [`reset`](crate::reset) and [`Ili9341`](crate::Ili9341) in the
//! crate root, and to their async counterparts in `asynch`. The async expansion only
//! differs in the `async` keyword on functions and `.await` after interface and delay
//! calls.

/// Generates `reset`, the `Ili9341` driver struct and its methods.
///
/// - `interface` / `delay`: the interface and delay traits to build on.
/// - `async`: empty, or `async` to make the methods async.
/// - `await`: empty, or `.await` to await interface and delay calls.
macro_rules! driver {
    (
        doc = $doc:literal,
        interface = $interface:ident,
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
        }

        impl<DI> Ili9341<DI> {
            /// Creates a driver in [`Portrait`](crate::Orientation::Portrait) orientation
            /// with [`Bgr`](crate::ColorOrder::Bgr) color order. Performs no bus access;
            /// call [`init`](Self::init) next.
            pub fn new(interface: DI) -> Self {
                let orientation = $crate::Orientation::Portrait;
                let madctl = orientation.apply($crate::registers::MemoryAccessControl::new().with_bgr(true));
                Self { interface, madctl, orientation }
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
            /// rate and gamma settings, 16-bit color, the stored orientation and color
            /// order, sleep out and display on.
            ///
            /// Takes about 240 ms. The frame memory content is undefined afterwards.
            pub $($asy)* fn init<D>(&mut self, delay: &mut D) -> Result<(), $crate::Error<DI::Error>>
            where
                D: $delay,
            {
                use $crate::Command;

                self.send_command(Command::SWRESET, &[]) $($aw)* ?;
                delay.delay_ms($crate::RESET_DELAY_MS) $($aw)*;
                for &(command, parameters) in $crate::PANEL_INIT {
                    self.interface
                        .send_command(command, parameters)
                        $($aw)*
                        .map_err($crate::Error::Interface)?;
                }
                self.send_command(Command::COLMOD, &[$crate::registers::PixelFormat::rgb565().into_bits()]) $($aw)* ?;
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
                let Some((columns, pages)) = self.window(x, y, width, height)? else {
                    return Ok(());
                };
                self.send_command($crate::Command::CASET, &columns) $($aw)* ?;
                self.send_command($crate::Command::PASET, &pages) $($aw)* ?;
                self.send_command($crate::Command::RAMWR, &[]) $($aw)* ?;
                self.interface.send_pixels(pixels) $($aw)* .map_err($crate::Error::Interface)
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
                // Per row: `width * height` overflows a 16-bit `usize` for a full screen.
                let pixels = (0..height).flat_map(|_| core::iter::repeat_n(color, width.into()));
                self.draw_pixels(x, y, width, height, pixels) $($aw)*
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
        }
    };
}
