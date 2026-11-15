//! [`DrawTarget`]s for the blocking driver: [`Rgb565`] on the driver itself, [`Rgb666`]
//! through [`Ili9341::rgb666`].

use crate::{Error, Ili9341, Interface, PixelData};
use embedded_graphics_core::pixelcolor::{IntoStorage, Rgb565, Rgb666, RgbColor};
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
        draw_iter(self, pixels, rgb565)
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Rgb565>,
    {
        fill_contiguous(self, area, colors.into_iter().map(rgb565))
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Rgb565) -> Result<(), Self::Error> {
        fill_solid(self, area, rgb565(color))
    }
}

impl<DI> Ili9341<DI> {
    /// Borrows the driver as a [`DrawTarget`] with [`Rgb666`] colors, for 18-bit color
    /// depth. See [`ColorDepth`](crate::ColorDepth).
    #[cfg_attr(docsrs, doc(cfg(feature = "embedded-graphics")))]
    pub fn rgb666(&mut self) -> Rgb666Target<'_, DI> {
        Rgb666Target(self)
    }
}

/// An [`Ili9341`] borrowed as a [`DrawTarget`] with [`Rgb666`] colors; see
/// [`Ili9341::rgb666`]. Clips to the screen.
#[cfg_attr(docsrs, doc(cfg(feature = "embedded-graphics")))]
#[derive(Debug)]
pub struct Rgb666Target<'a, DI>(&'a mut Ili9341<DI>);

impl<DI> OriginDimensions for Rgb666Target<'_, DI> {
    fn size(&self) -> Size {
        self.0.size()
    }
}

impl<DI: Interface> DrawTarget for Rgb666Target<'_, DI> {
    type Color = Rgb666;
    type Error = Error<DI::Error>;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Rgb666>>,
    {
        draw_iter(self.0, pixels, rgb666)
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Rgb666>,
    {
        fill_contiguous(self.0, area, colors.into_iter().map(rgb666))
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Rgb666) -> Result<(), Self::Error> {
        fill_solid(self.0, area, rgb666(color))
    }
}

fn rgb565(color: Rgb565) -> u16 {
    color.into_storage()
}

fn rgb666(color: Rgb666) -> [u8; 3] {
    [color.r(), color.g(), color.b()]
}

/// The most pixels [`draw_iter`] collects into one write.
const RUN_LEN: usize = 32;

/// Draws individual pixels. Pixels that continue a row to the right are collected into
/// one write, so a run costs one window setup instead of one per pixel.
fn draw_iter<DI, C, P, I>(
    display: &mut Ili9341<DI>,
    pixels: I,
    convert: impl Fn(C) -> P,
) -> Result<(), Error<DI::Error>>
where
    DI: Interface,
    C: PixelColor,
    P: PixelData + Default,
    I: IntoIterator<Item = Pixel<C>>,
{
    let bounds = display.bounding_box();
    let mut run = [P::default(); RUN_LEN];
    let mut len = 0;
    let mut start = Point::zero();
    for Pixel(point, color) in pixels {
        if !bounds.contains(point) {
            continue;
        }
        let continues = len > 0 && len < RUN_LEN && point == start + Point::new(len as i32, 0);
        if !continues {
            draw_run(display, start, &run[..len])?;
            start = point;
            len = 0;
        }
        run[len] = convert(color);
        len += 1;
    }
    draw_run(display, start, &run[..len])
}

/// Writes a horizontal run of on-screen pixels starting at `start`.
fn draw_run<DI, P>(
    display: &mut Ili9341<DI>,
    start: Point,
    run: &[P],
) -> Result<(), Error<DI::Error>>
where
    DI: Interface,
    P: PixelData,
{
    // On screen, so the coordinates and the length fit into u16.
    let (x, y, width) = (start.x as u16, start.y as u16, run.len() as u16);
    display.write_pixels(x, y, width, 1, run.iter().copied())
}

/// Fills an area with colors, row by row, dropping the off-screen part.
fn fill_contiguous<DI, P, I>(
    display: &mut Ili9341<DI>,
    area: &Rectangle,
    colors: I,
) -> Result<(), Error<DI::Error>>
where
    DI: Interface,
    P: PixelData,
    I: IntoIterator<Item = P>,
{
    let visible = area.intersection(&display.bounding_box());
    let Some((x, y, width, height)) = window(&visible) else {
        return Ok(());
    };
    // Both iterate row by row, so the visible points keep their order.
    let pixels = area
        .points()
        .zip(colors)
        .filter(|(point, _)| visible.contains(*point))
        .map(|(_, color)| color);
    display.write_pixels(x, y, width, height, pixels)
}

/// Fills the on-screen part of an area with one color.
fn fill_solid<DI, P>(
    display: &mut Ili9341<DI>,
    area: &Rectangle,
    color: P,
) -> Result<(), Error<DI::Error>>
where
    DI: Interface,
    P: PixelData,
{
    let visible = area.intersection(&display.bounding_box());
    match window(&visible) {
        Some((x, y, width, height)) => display.fill(x, y, width, height, color),
        None => Ok(()),
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
