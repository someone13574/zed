mod font_fallbacks;
mod font_features;
mod line;
mod line_layout;
mod line_wrapper;

use collections::FxHashMap;
pub use font_fallbacks::*;
pub use font_features::*;
use image::imageops::FilterType::Lanczos3;
use itertools::Itertools;
pub use line::*;
pub use line_layout::*;
pub use line_wrapper::*;
use parking_lot::{Mutex, RwLock, RwLockUpgradableReadGuard};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontData, Layout, LayoutContext, LineHeight,
    PositionedLayoutItem, StyleProperty,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    App, Bounds, DefiniteLength, DevicePixels, Hsla, Pixels, PlatformTextSystem, Point, Result,
    SharedString, Size, StrikethroughStyle, TextAlign, UnderlineStyle, Window, fill, point, px,
    size,
};
// use anyhow::{Context as _, anyhow};
// use collections::FxHashMap;
use core::fmt;
use derive_more::{Add, Deref, FromStr, Sub};
// use itertools::Itertools;
// use parking_lot::{Mutex, RwLock, RwLockUpgradableReadGuard};
use smallvec::SmallVec;
use std::{
    borrow::Cow,
    // cmp,
    fmt::{Debug, Display, Formatter},
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
    sync::Arc,
};

/// An opaque identifier for a specific font.
#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
#[repr(C)]
pub struct FontId(pub u64, pub u32);

/// An opaque identifier for a specific font family.
#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct FontFamilyId(pub usize);

pub(crate) const SUBPIXEL_VARIANTS_X: u8 = 4;

pub(crate) const SUBPIXEL_VARIANTS_Y: u8 =
    if cfg!(target_os = "windows") || cfg!(target_os = "linux") {
        1
    } else {
        SUBPIXEL_VARIANTS_X
    };

/// The GPUI text rendering sub system.
pub struct TextSystem {
    platform_text_system: Arc<dyn PlatformTextSystem>,
    font_ctx: Mutex<FontContext>,
    layout_ctx: Mutex<LayoutContext<Brush>>,
    // font_ids_by_font: RwLock<FxHashMap<Font, Result<FontId>>>,
    // font_metrics: RwLock<FxHashMap<FontId, FontMetrics>>,
    raster_bounds: RwLock<FxHashMap<RenderGlyphParams, Bounds<DevicePixels>>>,
    // wrapper_pool: Mutex<FxHashMap<FontIdWithSize, Vec<LineWrapper>>>,
    // font_runs_pool: Mutex<Vec<Vec<FontRun>>>,
    // fallback_font_stack: SmallVec<[Font; 2]>,
}

impl TextSystem {
    pub(crate) fn new(platform_text_system: Arc<dyn PlatformTextSystem>) -> Self {
        TextSystem {
            platform_text_system: platform_text_system,
            font_ctx: Mutex::default(),
            layout_ctx: Mutex::default(),
            // font_metrics: RwLock::default(),
            raster_bounds: RwLock::default(),
            // font_ids_by_font: RwLock::default(),
            // wrapper_pool: Mutex::default(),
            // font_runs_pool: Mutex::default(),
            // fallback_font_stack: smallvec![
            //     // TODO: Remove this when Linux have implemented setting fallbacks.
            //     font(".ZedMono"),
            //     font(".ZedSans"),
            //     font("Helvetica"),
            //     font("Segoe UI"),     // Windows
            //     font("Ubuntu"),       // Gnome (Ubuntu)
            //     font("Adwaita Sans"), // Gnome 47
            //     font("Cantarell"),    // Gnome
            //     font("Noto Sans"),    // KDE
            //     font("DejaVu Sans"),
            //     font("Arial"), // macOS, Windows
            // ],
        }
    }

    /// Get a list of all available font names from the operating system.
    pub fn all_font_names(&self) -> Vec<String> {
        todo!()
        // let mut names = self.platform_text_system.all_font_names();
        // names.extend(
        //     self.fallback_font_stack
        //         .iter()
        //         .map(|font| font.family.to_string()),
        // );
        // names.push(".SystemUIFont".to_string());
        // names.sort();
        // names.dedup();
        // names
    }

    /// Add a font's data to the text system.
    pub(crate) fn add_font(&self, font: FontData) -> Result<()> {
        self.platform_text_system.add_font(font)
    }

    // /// Get the FontId for the configure font family and style.
    // fn font_id(&self, font: &Font) -> Result<FontId> {
    //     fn clone_font_id_result(font_id: &Result<FontId>) -> Result<FontId> {
    //         match font_id {
    //             Ok(font_id) => Ok(*font_id),
    //             Err(err) => Err(anyhow!("{err}")),
    //         }
    //     }

    //     let font_id = self
    //         .font_ids_by_font
    //         .read()
    //         .get(font)
    //         .map(clone_font_id_result);
    //     if let Some(font_id) = font_id {
    //         font_id
    //     } else {
    //         let font_id = self.platform_text_system.font_id(font);
    //         self.font_ids_by_font
    //             .write()
    //             .insert(font.clone(), clone_font_id_result(&font_id));
    //         font_id
    //     }
    // }

    /// Get the Font for the Font Id.
    pub fn get_font_for_id(&self, _id: FontId) -> Option<Font> {
        todo!()
        // let lock = self.font_ids_by_font.read();
        // lock.iter()
        //     .filter_map(|(font, result)| match result {
        //         Ok(font_id) if *font_id == id => Some(font.clone()),
        //         _ => None,
        //     })
        //     .next()
    }

    /// Resolves the specified font, falling back to the default font stack if
    /// the font fails to load.
    ///
    /// # Panics
    ///
    /// Panics if the font and none of the fallbacks can be resolved.
    pub fn resolve_font(&self, _font: &Font) -> FontId {
        todo!()
        // if let Ok(font_id) = self.font_id(font) {
        //     return font_id;
        // }
        // for fallback in &self.fallback_font_stack {
        //     if let Ok(font_id) = self.font_id(fallback) {
        //         return font_id;
        //     }
        // }

        // panic!(
        //     "failed to resolve font '{}' or any of the fallbacks: {}",
        //     font.family,
        //     self.fallback_font_stack
        //         .iter()
        //         .map(|fallback| &fallback.family)
        //         .join(", ")
        // );
    }

    /// Get the bounding box for the given font and font size.
    /// A font's bounding box is the smallest rectangle that could enclose all glyphs
    /// in the font. superimposed over one another.
    pub fn bounding_box(&self, _font_id: FontId, _font_size: Pixels) -> Bounds<Pixels> {
        todo!()
        // self.read_metrics(font_id, |metrics| metrics.bounding_box(font_size))
    }

    /// Get the typographic bounds for the given character, in the given font and size.
    pub fn typographic_bounds(
        &self,
        _font_id: FontId,
        _font_size: Pixels,
        _character: char,
    ) -> Result<Bounds<Pixels>> {
        todo!()
        // let glyph_id = self
        //     .platform_text_system
        //     .glyph_for_char(font_id, character)
        //     .with_context(|| format!("glyph not found for character '{character}'"))?;
        // let bounds = self
        //     .platform_text_system
        //     .typographic_bounds(font_id, glyph_id)?;
        // Ok(self.read_metrics(font_id, |metrics| {
        //     (bounds / metrics.units_per_em as f32 * font_size.0).map(px)
        // }))
    }

    /// Get the advance width for the given character, in the given font and size.
    pub fn advance(&self, _font_id: FontId, _font_size: Pixels, _ch: char) -> Result<Size<Pixels>> {
        todo!()
        // let glyph_id = self
        //     .platform_text_system
        //     .glyph_for_char(font_id, ch)
        //     .with_context(|| format!("glyph not found for character '{ch}'"))?;
        // let result = self.platform_text_system.advance(font_id, glyph_id)?
        //     / self.units_per_em(font_id) as f32;

        // Ok(result * font_size)
    }

    /// Returns the width of an `em`.
    ///
    /// Uses the width of the `m` character in the given font and size.
    pub fn em_width(&self, _font_id: FontId, _font_size: Pixels) -> Result<Pixels> {
        todo!()
        // Ok(self.typographic_bounds(font_id, font_size, 'm')?.size.width)
    }

    /// Returns the advance width of an `em`.
    ///
    /// Uses the advance width of the `m` character in the given font and size.
    pub fn em_advance(&self, _font_id: FontId, _font_size: Pixels) -> Result<Pixels> {
        todo!()
        // Ok(self.advance(font_id, font_size, 'm')?.width)
    }

    /// Returns the width of an `ch`.
    ///
    /// Uses the width of the `0` character in the given font and size.
    pub fn ch_width(&self, _font_id: FontId, _font_size: Pixels) -> Result<Pixels> {
        todo!()
        // Ok(self.typographic_bounds(font_id, font_size, '0')?.size.width)
    }

    /// Returns the advance width of an `ch`.
    ///
    /// Uses the advance width of the `0` character in the given font and size.
    pub fn ch_advance(&self, _font_id: FontId, _font_size: Pixels) -> Result<Pixels> {
        todo!()
        // Ok(self.advance(font_id, font_size, '0')?.width)
    }

    /// Get the number of font size units per 'em square',
    /// Per MDN: "an abstract square whose height is the intended distance between
    /// lines of type in the same type size"
    pub fn units_per_em(&self, _font_id: FontId) -> u32 {
        todo!()
        // self.read_metrics(font_id, |metrics| metrics.units_per_em)
    }

    /// Get the height of a capital letter in the given font and size.
    pub fn cap_height(&self, _font_id: FontId, _font_size: Pixels) -> Pixels {
        todo!()
        // self.read_metrics(font_id, |metrics| metrics.cap_height(font_size))
    }

    /// Get the height of the x character in the given font and size.
    pub fn x_height(&self, _font_id: FontId, _font_size: Pixels) -> Pixels {
        todo!()
        // self.read_metrics(font_id, |metrics| metrics.x_height(font_size))
    }

    /// Get the recommended distance from the baseline for the given font
    pub fn ascent(&self, _font_id: FontId, _font_size: Pixels) -> Pixels {
        todo!()
        // self.read_metrics(font_id, |metrics| metrics.ascent(font_size))
    }

    /// Get the recommended distance below the baseline for the given font,
    /// in single spaced text.
    pub fn descent(&self, _font_id: FontId, _font_size: Pixels) -> Pixels {
        todo!()
        // self.read_metrics(font_id, |metrics| metrics.descent(font_size))
    }

    /// Get the recommended baseline offset for the given font and line height.
    pub fn baseline_offset(
        &self,
        font_id: FontId,
        font_size: Pixels,
        line_height: Pixels,
    ) -> Pixels {
        let ascent = self.ascent(font_id, font_size);
        let descent = self.descent(font_id, font_size);
        let padding_top = (line_height - ascent - descent) / 2.;
        padding_top + ascent
    }

    // fn read_metrics<T>(&self, font_id: FontId, read: impl FnOnce(&FontMetrics) -> T) -> T {
    //     let lock = self.font_metrics.upgradable_read();

    //     if let Some(metrics) = lock.get(&font_id) {
    //         read(metrics)
    //     } else {
    //         let mut lock = RwLockUpgradableReadGuard::upgrade(lock);
    //         let metrics = lock
    //             .entry(font_id)
    //             .or_insert_with(|| self.platform_text_system.font_metrics(font_id));
    //         read(metrics)
    //     }
    // }

    /// Returns a handle to a line wrapper, for the given font and font size.
    #[deprecated]
    pub fn line_wrapper(self: &Arc<Self>, _font: Font, _font_size: Pixels) -> LineWrapperHandle {
        todo!()
        // let lock = &mut self.wrapper_pool.lock();
        // let font_id = self.resolve_font(&font);
        // let wrappers = lock
        //     .entry(FontIdWithSize { font_id, font_size })
        //     .or_default();
        // let wrapper = wrappers.pop().unwrap_or_else(|| {
        //     LineWrapper::new(font_id, font_size, self.platform_text_system.clone())
        // });

        // LineWrapperHandle {
        //     wrapper: Some(wrapper),
        //     text_system: self.clone(),
        // }
    }

    /// Get the rasterized size and location of a specific, rendered glyph.
    pub(crate) fn raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        let raster_bounds = self.raster_bounds.upgradable_read();
        if let Some(bounds) = raster_bounds.get(params) {
            Ok(*bounds)
        } else {
            let mut raster_bounds = RwLockUpgradableReadGuard::upgrade(raster_bounds);
            let bounds = self.platform_text_system.glyph_raster_bounds(params)?;
            raster_bounds.insert(params.clone(), bounds);
            Ok(bounds)
        }
    }

    pub(crate) fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        let raster_bounds = self.raster_bounds(params)?;
        self.platform_text_system
            .rasterize_glyph(params, raster_bounds)
    }
}

/// The GPUI text layout subsystem.
#[derive(Deref)]
pub struct WindowTextSystem {
    // line_layout_cache: LineLayoutCache,
    #[deref]
    text_system: Arc<TextSystem>,
}

impl WindowTextSystem {
    pub(crate) fn new(text_system: Arc<TextSystem>) -> Self {
        Self {
            // line_layout_cache: LineLayoutCache::new(text_system.platform_text_system.clone()),
            text_system,
        }
    }

    // pub(crate) fn layout_index(&self) -> LineLayoutIndex {
    //     todo!()
    //     // self.line_layout_cache.layout_index()
    // }

    // pub(crate) fn reuse_layouts(&self, _index: Range<LineLayoutIndex>) {
    //     todo!()
    //     // self.line_layout_cache.reuse_layouts(index)
    // }

    // pub(crate) fn truncate_layouts(&self, _index: LineLayoutIndex) {
    //     todo!()
    //     // self.line_layout_cache.truncate_layouts(index)
    // }

    /// Shape a multi line string of text, at the given font_size, for painting to the screen.
    /// Subsets of the text can be styled independently with the `runs` parameter.
    /// If `wrap_width` is provided, the line breaks will be adjusted to fit within the given width.
    pub fn shape_text(
        &self,
        text: SharedString,
        font_size: Pixels,
        line_height: Pixels,
        runs: &[TextRun],
        wrap_width: Option<Pixels>,
        line_clamp: Option<usize>,
    ) -> ShapedText {
        let mut layout_ctx = self.layout_ctx.lock();
        let mut font_ctx = self.font_ctx.lock();

        let mut builder = layout_ctx.ranged_builder(&mut font_ctx, &text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(font_size.0));
        builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(
            line_height.0, // TODO_parley: allow metric here
        )));

        let mut offset = 0;
        for run in runs.iter().filter(|run| run.len > 0) {
            let range = offset..offset + run.len;
            offset += run.len;

            builder.push(
                StyleProperty::FontStyle(match run.font.style {
                    FontStyle::Normal => parley::FontStyle::Normal,
                    FontStyle::Italic => parley::FontStyle::Italic,
                    FontStyle::Oblique => parley::FontStyle::Oblique(None),
                }),
                range.clone(),
            );

            builder.push(
                StyleProperty::FontWeight(parley::FontWeight::new(run.font.weight.0)),
                range.clone(),
            );

            builder.push(
                StyleProperty::Brush(Brush {
                    color: run.color,
                    background: run.background_color,
                    underline: run.underline.map(|mut underline| {
                        underline.color = underline.color.or(Some(run.color));
                        underline
                    }),
                    strikethrough: run.strikethrough.map(|mut strikethrough| {
                        strikethrough.color = strikethrough.color.or(Some(run.color));
                        strikethrough
                    }),
                }),
                range.clone(),
            );
        }

        let mut layout = builder.build(&text);
        layout.break_all_lines(wrap_width.map(|px| px.0));

        ShapedText {
            layout,
            text,
            line_height,
        }
        // let mut runs = runs.iter().filter(|run| run.len > 0).cloned().peekable();
        // let mut font_runs = self.font_runs_pool.lock().pop().unwrap_or_default();

        // let mut lines = SmallVec::new();
        // let mut max_wrap_lines = line_clamp;
        // let mut wrapped_lines = 0;

        // let mut process_line = |line_text: SharedString, line_start, line_end| {
        //     font_runs.clear();

        //     let mut decoration_runs = SmallVec::<[DecorationRun; 32]>::new();
        //     let mut run_start = line_start;
        //     while run_start < line_end {
        //         let Some(run) = runs.peek_mut() else {
        //             log::warn!("`TextRun`s do not cover the entire to be shaped text");
        //             break;
        //         };

        //         let run_len_within_line = cmp::min(line_end - run_start, run.len);

        //         let decoration_changed = if let Some(last_run) = decoration_runs.last_mut()
        //             && last_run.color == run.color
        //             && last_run.underline == run.underline
        //             && last_run.strikethrough == run.strikethrough
        //             && last_run.background_color == run.background_color
        //         {
        //             last_run.len += run_len_within_line as u32;
        //             false
        //         } else {
        //             decoration_runs.push(DecorationRun {
        //                 len: run_len_within_line as u32,
        //                 color: run.color,
        //                 background_color: run.background_color,
        //                 underline: run.underline,
        //                 strikethrough: run.strikethrough,
        //             });
        //             true
        //         };

        //         let font_id = self.resolve_font(&run.font);
        //         if let Some(font_run) = font_runs.last_mut()
        //             && font_id == font_run.font_id
        //             && !decoration_changed
        //         {
        //             font_run.len += run_len_within_line;
        //         } else {
        //             font_runs.push(FontRun {
        //                 len: run_len_within_line,
        //                 font_id,
        //             });
        //         }

        //         // Preserve the remainder of the run for the next line
        //         run.len -= run_len_within_line;
        //         if run.len == 0 {
        //             runs.next();
        //         }
        //         run_start += run_len_within_line;
        //     }

        //     let layout = self.line_layout_cache.layout_wrapped_line(
        //         &line_text,
        //         font_size,
        //         &font_runs,
        //         wrap_width,
        //         max_wrap_lines.map(|max| max.saturating_sub(wrapped_lines)),
        //     );
        //     wrapped_lines += layout.wrap_boundaries.len();

        //     lines.push(WrappedLine {
        //         layout,
        //         decoration_runs,
        //         text: line_text,
        //     });

        //     // Skip `\n` character.
        //     if let Some(run) = runs.peek_mut() {
        //         run.len -= 1;
        //         if run.len == 0 {
        //             runs.next();
        //         }
        //     }
        // };

        // let mut split_lines = text.split('\n');

        // // Special case single lines to prevent allocating a sharedstring
        // if let Some(first_line) = split_lines.next()
        //     && let Some(second_line) = split_lines.next()
        // {
        //     let mut line_start = 0;
        //     process_line(
        //         SharedString::new(first_line),
        //         line_start,
        //         line_start + first_line.len(),
        //     );
        //     line_start += first_line.len() + '\n'.len_utf8();
        //     process_line(
        //         SharedString::new(second_line),
        //         line_start,
        //         line_start + second_line.len(),
        //     );
        //     for line_text in split_lines {
        //         line_start += line_text.len() + '\n'.len_utf8();
        //         process_line(
        //             SharedString::new(line_text),
        //             line_start,
        //             line_start + line_text.len(),
        //         );
        //     }
        // } else {
        //     let end = text.len();
        //     process_line(text, 0, end);
        // }

        // self.font_runs_pool.lock().push(font_runs);

        // Ok(lines)
    }

    pub(crate) fn finish_frame(&self) {
        // self.line_layout_cache.finish_frame()
    }

    /// Layout the given line of text, at the given font_size.
    /// Subsets of the line can be styled independently with the `runs` parameter.
    /// Generally, you should prefer to use [`Self::shape_line`] instead, which
    /// can be painted directly.
    pub fn layout_line(
        &self,
        _text: &str,
        _font_size: Pixels,
        _runs: &[TextRun],
        _force_width: Option<Pixels>,
    ) -> Arc<LineLayout> {
        todo!()
        // let mut last_run = None::<&TextRun>;
        // let mut last_font: Option<FontId> = None;
        // let mut font_runs = self.font_runs_pool.lock().pop().unwrap_or_default();
        // font_runs.clear();

        // for run in runs.iter() {
        //     let decoration_changed = if let Some(last_run) = last_run
        //         && last_run.color == run.color
        //         && last_run.underline == run.underline
        //         && last_run.strikethrough == run.strikethrough
        //     // we do not consider differing background color relevant, as it does not affect glyphs
        //     // && last_run.background_color == run.background_color
        //     {
        //         false
        //     } else {
        //         last_run = Some(run);
        //         true
        //     };

        //     if let Some(font_run) = font_runs.last_mut()
        //         && Some(font_run.font_id) == last_font
        //         && !decoration_changed
        //     {
        //         font_run.len += run.len;
        //     } else {
        //         let font_id = self.resolve_font(&run.font);
        //         last_font = Some(font_id);
        //         font_runs.push(FontRun {
        //             len: run.len,
        //             font_id,
        //         });
        //     }
        // }

        // let layout = self.line_layout_cache.layout_line(
        //     &SharedString::new(text),
        //     font_size,
        //     &font_runs,
        //     force_width,
        // );

        // self.font_runs_pool.lock().push(font_runs);

        // layout
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Brush {
    color: Hsla,
    background: Option<Hsla>,
    underline: Option<UnderlineStyle>,
    strikethrough: Option<StrikethroughStyle>,
}

impl Default for Brush {
    fn default() -> Self {
        Self {
            color: gpui::black(),
            background: None,
            underline: None,
            strikethrough: None,
        }
    }
}

/// Text which has been shaped and laid-out
pub struct ShapedText {
    layout: Layout<Brush>,
    /// The text that was shaped
    pub text: SharedString,
    line_height: Pixels,
}

impl ShapedText {
    /// The size of the whole wrapped text. This can span multiple lines if there are multiple wrap boundaries.
    pub fn size(&self) -> Size<Pixels> {
        size(self.layout.width().into(), self.layout.height().into())
    }

    /// Sets the alignment of the shaped text within a container
    pub fn align(&mut self, container_width: Pixels, alignment: TextAlign) {
        self.layout.align(
            Some(container_width.0),
            match alignment {
                TextAlign::Left => Alignment::Left,
                TextAlign::Center => Alignment::Center,
                TextAlign::Right => Alignment::Right,
            },
            AlignmentOptions {
                align_when_overflowing: true,
            },
        );
    }

    /// The utf8 byte-index for the character at the given coordinates
    pub fn index_for_position(&self, position: Point<Pixels>) -> Option<usize> {
        let line_count = self.layout.len();
        if position.y < px(0.0) || position.y > self.size().height || line_count == 0 {
            return None;
        };

        if line_count == 0 {
            return None;
        }

        let line_idx = ((position.y / self.line_height).floor() as usize).min(line_count - 1);
        let line = self.layout.get(line_idx)?;

        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };

            let mut offset = px(glyph_run.offset());
            for cluster in glyph_run.run().clusters() {
                let next = offset + cluster.advance().into();
                if position.x < next {
                    return Some(cluster.text_range().start);
                }

                offset = next;
            }
        }

        None
    }

    /// The utf8 byte-index for the character closest to the given coordinates
    pub fn closest_index_for_position(&self, position: Point<Pixels>) -> usize {
        let line_count = self.layout.len();
        if position.y < px(0.0) || line_count == 0 {
            return 0;
        } else if position.y > self.size().height {
            return self.text.len();
        }

        let line_idx =
            ((position.y / self.line_height).floor() as usize).min(self.layout.lines().count());
        let Some(line) = self.layout.get(line_idx) else {
            return self.text.len();
        };

        let mut last = self.text.len();
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };

            let mut offset = px(glyph_run.offset());
            for cluster in glyph_run.run().clusters() {
                if position.x < offset + px(cluster.advance() / 2.0) {
                    return cluster.text_range().start;
                } else {
                    last = cluster.text_range().end;
                }

                offset += cluster.advance().into();
            }
        }

        last
    }

    /// The position of the character at the given utf8 byte-index
    pub fn position_for_index(&self, index: usize) -> Point<Pixels> {
        let mut last = px(self.layout.width());
        for line in self.layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };

                let mut origin = point(px(glyph_run.offset()), px(glyph_run.baseline()));
                for cluster in glyph_run.run().clusters() {
                    let next = origin.x + cluster.advance().into();

                    if cluster.text_range().contains(&index) {
                        return origin;
                    } else {
                        last = next;
                    }

                    origin.x = next;
                }
            }
        }

        // Can't return layout.width() directly since we need to account for trailing spaces
        point(last, self.layout.height().into())
    }

    /// Paints the text to the window
    pub fn paint(&self, origin: Point<Pixels>, window: &mut Window) {
        let bounds = Bounds::new(
            origin,
            size(self.layout.width().into(), self.layout.height().into()),
        );

        window.paint_layer(bounds, |window| {
            for (line_idx, line) in self.layout.lines().enumerate() {
                for item in line.items() {
                    match item {
                        PositionedLayoutItem::GlyphRun(glyph_run) => {
                            let run_origin =
                                origin + point(px(glyph_run.offset()), px(glyph_run.baseline()));
                            let mut glyph_origin = run_origin;

                            let font = glyph_run.run().font();
                            let font_size = px(glyph_run.run().font_size());
                            window.text_system().add_font(font.clone()).unwrap();

                            let brush = &glyph_run.style().brush;
                            if let Some(bg) = brush.background {
                                window.paint_quad(fill(
                                    Bounds::new(
                                        origin
                                            + point(
                                                px(glyph_run.offset()),
                                                self.line_height * line_idx,
                                            ),
                                        size(glyph_run.advance().into(), self.line_height),
                                    ),
                                    bg,
                                ));
                            }

                            for glyph in glyph_run.glyphs() {
                                window
                                    .paint_glyph(
                                        glyph_origin,
                                        FontId(font.data.id(), font.index),
                                        GlyphId(glyph.id),
                                        font_size,
                                        brush.color,
                                    )
                                    .unwrap();

                                glyph_origin.x += px(glyph.advance);
                            }

                            if let Some(underline) = brush.underline {
                                window.paint_underline(
                                    run_origin
                                        - point(
                                            px(0.0),
                                            glyph_run.run().metrics().underline_offset.into(),
                                        ),
                                    glyph_run.advance().into(),
                                    &underline,
                                );
                            }

                            if let Some(strikethrough) = brush.strikethrough {
                                window.paint_strikethrough(
                                    run_origin
                                        - point(
                                            px(0.0),
                                            glyph_run.run().metrics().strikethrough_offset.into(),
                                        ),
                                    glyph_run.advance().into(),
                                    &strikethrough,
                                );
                            }
                        }
                        PositionedLayoutItem::InlineBox(_positioned_inline_box) => todo!(),
                    }
                }
            }
        });
    }
}

// #[derive(Hash, Eq, PartialEq)]
// struct FontIdWithSize {
//     font_id: FontId,
//     font_size: Pixels,
// }

/// A handle into the text system, which can be used to compute the wrapped layout of text
pub struct LineWrapperHandle {
    wrapper: Option<LineWrapper>,
    _text_system: Arc<TextSystem>,
}

impl Drop for LineWrapperHandle {
    fn drop(&mut self) {
        todo!()
        // let mut state = self.text_system.wrapper_pool.lock();
        // let wrapper = self.wrapper.take().unwrap();
        // state
        //     .get_mut(&FontIdWithSize {
        //         font_id: wrapper.font_id,
        //         font_size: wrapper.font_size,
        //     })
        //     .unwrap()
        //     .push(wrapper);
    }
}

impl Deref for LineWrapperHandle {
    type Target = LineWrapper;

    fn deref(&self) -> &Self::Target {
        self.wrapper.as_ref().unwrap()
    }
}

impl DerefMut for LineWrapperHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.wrapper.as_mut().unwrap()
    }
}

/// The degree of blackness or stroke thickness of a font. This value ranges from 100.0 to 900.0,
/// with 400.0 as normal.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Add, Sub, FromStr)]
#[serde(transparent)]
pub struct FontWeight(pub f32);

impl Display for FontWeight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<f32> for FontWeight {
    fn from(weight: f32) -> Self {
        FontWeight(weight)
    }
}

impl Default for FontWeight {
    #[inline]
    fn default() -> FontWeight {
        FontWeight::NORMAL
    }
}

impl Hash for FontWeight {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(u32::from_be_bytes(self.0.to_be_bytes()));
    }
}

impl Eq for FontWeight {}

impl FontWeight {
    /// Thin weight (100), the thinnest value.
    pub const THIN: FontWeight = FontWeight(100.0);
    /// Extra light weight (200).
    pub const EXTRA_LIGHT: FontWeight = FontWeight(200.0);
    /// Light weight (300).
    pub const LIGHT: FontWeight = FontWeight(300.0);
    /// Normal (400).
    pub const NORMAL: FontWeight = FontWeight(400.0);
    /// Medium weight (500, higher than normal).
    pub const MEDIUM: FontWeight = FontWeight(500.0);
    /// Semibold weight (600).
    pub const SEMIBOLD: FontWeight = FontWeight(600.0);
    /// Bold weight (700).
    pub const BOLD: FontWeight = FontWeight(700.0);
    /// Extra-bold weight (800).
    pub const EXTRA_BOLD: FontWeight = FontWeight(800.0);
    /// Black weight (900), the thickest value.
    pub const BLACK: FontWeight = FontWeight(900.0);

    /// All of the font weights, in order from thinnest to thickest.
    pub const ALL: [FontWeight; 9] = [
        Self::THIN,
        Self::EXTRA_LIGHT,
        Self::LIGHT,
        Self::NORMAL,
        Self::MEDIUM,
        Self::SEMIBOLD,
        Self::BOLD,
        Self::EXTRA_BOLD,
        Self::BLACK,
    ];
}

impl schemars::JsonSchema for FontWeight {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "FontWeight".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        use schemars::json_schema;
        json_schema!({
            "type": "number",
            "minimum": Self::THIN,
            "maximum": Self::BLACK,
            "default": Self::default(),
            "description": "Font weight value between 100 (thin) and 900 (black)"
        })
    }
}

/// Allows italic or oblique faces to be selected.
#[derive(Clone, Copy, Eq, PartialEq, Debug, Hash, Default, Serialize, Deserialize, JsonSchema)]
pub enum FontStyle {
    /// A face that is neither italic not obliqued.
    #[default]
    Normal,
    /// A form that is generally cursive in nature.
    Italic,
    /// A typically-sloped version of the regular face.
    Oblique,
}

impl Display for FontStyle {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

/// A styled run of text, for use in [`crate::TextLayout`].
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TextRun {
    /// A number of utf8 bytes
    pub len: usize,
    /// The font to use for this run.
    pub font: Font,
    /// The color
    pub color: Hsla,
    /// The background color (if any)
    pub background_color: Option<Hsla>,
    /// The underline style (if any)
    pub underline: Option<UnderlineStyle>,
    /// The strikethrough style (if any)
    pub strikethrough: Option<StrikethroughStyle>,
}

#[cfg(all(target_os = "macos", test))]
impl TextRun {
    fn with_len(&self, len: usize) -> Self {
        let mut this = self.clone();
        this.len = len;
        this
    }
}

/// An identifier for a specific glyph, as returned by [`WindowTextSystem::layout_line`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct GlyphId(pub(crate) u32);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RenderGlyphParams {
    pub(crate) font_id: FontId,
    pub(crate) glyph_id: GlyphId,
    pub(crate) font_size: Pixels,
    pub(crate) subpixel_variant: Point<u8>,
    pub(crate) scale_factor: f32,
    pub(crate) is_emoji: bool,
}

impl Eq for RenderGlyphParams {}

impl Hash for RenderGlyphParams {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.font_id.0.hash(state);
        self.glyph_id.0.hash(state);
        self.font_size.0.to_bits().hash(state);
        self.subpixel_variant.hash(state);
        self.scale_factor.to_bits().hash(state);
        self.is_emoji.hash(state);
    }
}

/// The configuration details for identifying a specific font.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Font {
    /// The font family name.
    ///
    /// The special name ".SystemUIFont" is used to identify the system UI font, which varies based on platform.
    pub family: SharedString,

    /// The font features to use.
    pub features: FontFeatures,

    /// The fallbacks fonts to use.
    pub fallbacks: Option<FontFallbacks>,

    /// The font weight.
    pub weight: FontWeight,

    /// The font style.
    pub style: FontStyle,
}

impl Default for Font {
    fn default() -> Self {
        font(".SystemUIFont")
    }
}

/// Get a [`Font`] for a given name.
pub fn font(family: impl Into<SharedString>) -> Font {
    Font {
        family: family.into(),
        features: FontFeatures::default(),
        weight: FontWeight::default(),
        style: FontStyle::default(),
        fallbacks: None,
    }
}

impl Font {
    /// Set this Font to be bold
    pub fn bold(mut self) -> Self {
        self.weight = FontWeight::BOLD;
        self
    }

    /// Set this Font to be italic
    pub fn italic(mut self) -> Self {
        self.style = FontStyle::Italic;
        self
    }
}

/// A struct for storing font metrics.
/// It is used to define the measurements of a typeface.
#[derive(Clone, Copy, Debug)]
pub struct FontMetrics {
    /// The number of font units that make up the "em square",
    /// a scalable grid for determining the size of a typeface.
    pub(crate) units_per_em: u32,

    /// The vertical distance from the baseline of the font to the top of the glyph covers.
    pub(crate) ascent: f32,

    /// The vertical distance from the baseline of the font to the bottom of the glyph covers.
    pub(crate) descent: f32,

    /// The recommended additional space to add between lines of type.
    pub(crate) line_gap: f32,

    /// The suggested position of the underline.
    pub(crate) underline_position: f32,

    /// The suggested thickness of the underline.
    pub(crate) underline_thickness: f32,

    /// The height of a capital letter measured from the baseline of the font.
    pub(crate) cap_height: f32,

    /// The height of a lowercase x.
    pub(crate) x_height: f32,

    /// The outer limits of the area that the font covers.
    /// Corresponds to the xMin / xMax / yMin / yMax values in the OpenType `head` table
    pub(crate) bounding_box: Bounds<f32>,
}

impl FontMetrics {
    /// Returns the vertical distance from the baseline of the font to the top of the glyph covers in pixels.
    pub fn ascent(&self, font_size: Pixels) -> Pixels {
        Pixels((self.ascent / self.units_per_em as f32) * font_size.0)
    }

    /// Returns the vertical distance from the baseline of the font to the bottom of the glyph covers in pixels.
    pub fn descent(&self, font_size: Pixels) -> Pixels {
        Pixels((self.descent / self.units_per_em as f32) * font_size.0)
    }

    /// Returns the recommended additional space to add between lines of type in pixels.
    pub fn line_gap(&self, font_size: Pixels) -> Pixels {
        Pixels((self.line_gap / self.units_per_em as f32) * font_size.0)
    }

    /// Returns the suggested position of the underline in pixels.
    pub fn underline_position(&self, font_size: Pixels) -> Pixels {
        Pixels((self.underline_position / self.units_per_em as f32) * font_size.0)
    }

    /// Returns the suggested thickness of the underline in pixels.
    pub fn underline_thickness(&self, font_size: Pixels) -> Pixels {
        Pixels((self.underline_thickness / self.units_per_em as f32) * font_size.0)
    }

    /// Returns the height of a capital letter measured from the baseline of the font in pixels.
    pub fn cap_height(&self, font_size: Pixels) -> Pixels {
        Pixels((self.cap_height / self.units_per_em as f32) * font_size.0)
    }

    /// Returns the height of a lowercase x in pixels.
    pub fn x_height(&self, font_size: Pixels) -> Pixels {
        Pixels((self.x_height / self.units_per_em as f32) * font_size.0)
    }

    /// Returns the outer limits of the area that the font covers in pixels.
    pub fn bounding_box(&self, font_size: Pixels) -> Bounds<Pixels> {
        (self.bounding_box / self.units_per_em as f32 * font_size.0).map(px)
    }
}

#[allow(unused)]
pub(crate) fn font_name_with_fallbacks<'a>(name: &'a str, system: &'a str) -> &'a str {
    // Note: the "Zed Plex" fonts were deprecated as we are not allowed to use "Plex"
    // in a derived font name. They are essentially indistinguishable from IBM Plex/Lilex,
    // and so retained here for backward compatibility.
    match name {
        ".SystemUIFont" => system,
        ".ZedSans" | "Zed Plex Sans" => "IBM Plex Sans",
        ".ZedMono" | "Zed Plex Mono" => "Lilex",
        _ => name,
    }
}
