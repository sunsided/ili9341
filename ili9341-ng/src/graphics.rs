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
        let bounds = self.bounding_box();
        for Pixel(point, color) in pixels {
            if bounds.contains(point) {
                // In bounds, so the coordinates fit into u16.
                self.set_pixel(point.x as u16, point.y as u16, color.into_storage())?;
            }
        }
        Ok(())
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
