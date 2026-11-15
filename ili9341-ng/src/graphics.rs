//! [`DrawTarget`] for the blocking driver.

use crate::{Error, Ili9341, Interface};
use embedded_graphics_core::pixelcolor::{IntoStorage, Rgb565};
use embedded_graphics_core::prelude::*;
use embedded_graphics_core::primitives::Rectangle;

#[cfg_attr(docsrs, doc(cfg(feature = "embedded-graphics")))]
impl<DI> OriginDimensions for Ili9341<DI> {
    fn size(&self) -> Size {
        let (width, height) = self.dimensions();
        Size::new(width.into(), height.into())
    }
}

/// Clips to the screen: pixels outside it are dropped.
#[cfg_attr(docsrs, doc(cfg(feature = "embedded-graphics")))]
impl<DI: Interface> DrawTarget for Ili9341<DI> {
    type Color = Rgb565;
    type Error = Error<DI::Error>;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Rgb565>>,
    {
        // Pixels that continue a row to the right are collected into one write, so a
        // run costs one window setup instead of one per pixel.
        let bounds = self.bounding_box();
        let mut run = [0; RUN_LEN];
        let mut len = 0;
        let mut start = Point::zero();
        for Pixel(point, color) in pixels {
            if !bounds.contains(point) {
                continue;
            }
            let continues = len > 0 && len < RUN_LEN && point == start + Point::new(len as i32, 0);
            if !continues {
                self.draw_run(start, &run[..len])?;
                start = point;
                len = 0;
            }
            run[len] = color.into_storage();
            len += 1;
        }
        self.draw_run(start, &run[..len])
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Rgb565>,
    {
        let visible = area.intersection(&self.bounding_box());
        let Some((x, y, width, height)) = window(&visible) else {
            return Ok(());
        };
        // Both iterate row by row, so the visible points keep their order.
        let pixels = area
            .points()
            .zip(colors)
            .filter(|(point, _)| visible.contains(*point))
            .map(|(_, color)| color.into_storage());
        self.draw_pixels(x, y, width, height, pixels)
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Rgb565) -> Result<(), Self::Error> {
        let visible = area.intersection(&self.bounding_box());
        match window(&visible) {
            Some((x, y, width, height)) => {
                self.fill_rect(x, y, width, height, color.into_storage())
            }
            None => Ok(()),
        }
    }

    fn clear(&mut self, color: Rgb565) -> Result<(), Self::Error> {
        self.fill_screen(color.into_storage())
    }
}

/// The most pixels [`DrawTarget::draw_iter`] collects into one write.
const RUN_LEN: usize = 32;

impl<DI: Interface> Ili9341<DI> {
    /// Writes a horizontal run of on-screen pixels starting at `start`.
    fn draw_run(&mut self, start: Point, run: &[u16]) -> Result<(), Error<DI::Error>> {
        // On screen, so the coordinates and the length fit into u16.
        let (x, y, width) = (start.x as u16, start.y as u16, run.len() as u16);
        self.draw_pixels(x, y, width, 1, run.iter().copied())
    }
}

/// Converts an on-screen rectangle into `(x, y, width, height)`, or `None` if it is empty.
fn window(area: &Rectangle) -> Option<(u16, u16, u16, u16)> {
    if area.is_zero_sized() {
        return None;
    }
    // The rectangle lies within the screen, so all values fit into u16.
    Some((
        area.top_left.x as u16,
        area.top_left.y as u16,
        area.size.width as u16,
        area.size.height as u16,
    ))
}
