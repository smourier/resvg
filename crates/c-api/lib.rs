// Copyright 2020 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! C bindings.

#![allow(non_camel_case_types)]
#![warn(missing_docs)]
#![warn(missing_copy_implementations)]

use std::any::Any;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;
use std::slice;

use resvg::tiny_skia;
use resvg::usvg;

fn log_panic_payload(payload: &(dyn Any + Send)) {
    if let Some(message) = payload.downcast_ref::<&str>() {
        log::error!("caught panic in resvg C API: {}", message);
    } else if let Some(message) = payload.downcast_ref::<String>() {
        log::error!("caught panic in resvg C API: {}", message);
    } else {
        log::error!("caught panic in resvg C API: non-string panic payload");
    }
}

fn ffi_try<T>(context: &'static str, default: T, f: impl FnOnce() -> T) -> T {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(payload) => {
            log::error!("panic while executing {}", context);
            log_panic_payload(payload.as_ref());
            default
        }
    }
}

#[inline]
fn cast_opt(opt: *mut resvg_options) -> Option<&'static mut usvg::Options<'static>> {
    if opt.is_null() {
        log::error!("resvg_options pointer is null");
        return None;
    }

    Some(unsafe { &mut (*opt).options })
}

#[inline]
fn cast_tree(tree: *const resvg_render_tree) -> Option<&'static resvg_render_tree> {
    if tree.is_null() {
        log::error!("resvg_render_tree pointer is null");
        return None;
    }

    Some(unsafe { &*tree })
}

fn cstr_to_str(text: *const c_char) -> Option<&'static str> {
    if text.is_null() {
        log::error!("received null string pointer");
        return None;
    }

    let text = unsafe { CStr::from_ptr(text) };

    match text.to_str() {
        Ok(text) => Some(text),
        Err(err) => {
            log::error!("received non UTF-8 string: {}", err);
            None
        }
    }
}

/// @brief List of possible errors.
#[repr(C)]
#[derive(Copy, Clone)]
pub enum resvg_error {
    /// Everything is ok.
    OK = 0,
    /// Only UTF-8 content are supported.
    NOT_AN_UTF8_STR,
    /// SVGZ decoding is unsupported.
    SVGZ_UNSUPPORTED,
    /// Failed to open the provided file.
    FILE_OPEN_FAILED,
    /// Compressed SVG must use the GZip algorithm.
    MALFORMED_GZIP,
    /// We do not allow SVG with more than 1_000_000 elements for security reasons.
    ELEMENTS_LIMIT_REACHED,
    /// SVG doesn't have a valid size.
    ///
    /// Occurs when width and/or height are <= 0.
    ///
    /// Also occurs if width, height and viewBox are not set.
    INVALID_SIZE,
    /// Failed to parse an SVG data.
    PARSING_FAILED,
}

/// @brief A rectangle representation.
#[repr(C)]
#[allow(missing_docs)]
#[derive(Copy, Clone)]
pub struct resvg_rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// @brief A size representation.
#[repr(C)]
#[allow(missing_docs)]
#[derive(Copy, Clone)]
pub struct resvg_size {
    pub width: f32,
    pub height: f32,
}

/// @brief A 2D transform representation.
#[repr(C)]
#[allow(missing_docs)]
#[derive(Copy, Clone)]
pub struct resvg_transform {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl resvg_transform {
    #[inline]
    fn to_tiny_skia(&self) -> tiny_skia::Transform {
        tiny_skia::Transform::from_row(self.a, self.b, self.c, self.d, self.e, self.f)
    }
}

/// @brief Creates an identity transform.
#[unsafe(no_mangle)]
pub extern "C" fn resvg_transform_identity() -> resvg_transform {
    ffi_try(
        "resvg_transform_identity",
        resvg_transform {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        },
        || resvg_transform {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        },
    )
}

/// @brief Initializes the library log.
///
/// Use it if you want to see any warnings.
///
/// Must be called only once.
///
/// All warnings will be printed to the `stderr`.
#[unsafe(no_mangle)]
pub extern "C" fn resvg_init_log() {
    ffi_try("resvg_init_log", (), || {
        if let Ok(()) = log::set_logger(&LOGGER) {
            log::set_max_level(log::LevelFilter::Error);
        }
    })
}

/// @brief An SVG to #resvg_render_tree conversion options.
///
/// Also, contains a fonts database used during text to path conversion.
/// The database is empty by default.
pub struct resvg_options {
    options: usvg::Options<'static>,
}

/// @brief Creates a new #resvg_options object.
///
/// Should be destroyed via #resvg_options_destroy.
#[unsafe(no_mangle)]
pub extern "C" fn resvg_options_create() -> *mut resvg_options {
    ffi_try("resvg_options_create", ptr::null_mut(), || {
        Box::into_raw(Box::new(resvg_options {
            options: usvg::Options::default(),
        }))
    })
}

/// @brief Sets a directory that will be used during relative paths resolving.
///
/// Expected to be the same as the directory that contains the SVG file,
/// but can be set to any.
///
/// Must be UTF-8. Can be set to NULL.
///
/// Default: NULL
#[unsafe(no_mangle)]
pub extern "C" fn resvg_options_set_resources_dir(opt: *mut resvg_options, path: *const c_char) {
    ffi_try("resvg_options_set_resources_dir", (), || {
        let Some(opt) = cast_opt(opt) else {
            return;
        };

        if path.is_null() {
            opt.resources_dir = None;
        } else {
            opt.resources_dir = cstr_to_str(path).map(Into::into);
        }
    })
}

/// @brief Sets the target DPI.
///
/// Impact units conversion.
///
/// Default: 96
#[unsafe(no_mangle)]
pub extern "C" fn resvg_options_set_dpi(opt: *mut resvg_options, dpi: f32) {
    ffi_try("resvg_options_set_dpi", (), || {
        let Some(opt) = cast_opt(opt) else {
            return;
        };

        opt.dpi = dpi;
    })
}

/// @brief Provides the content of a stylesheet that will be used when resolving CSS attributes.
///
/// Must be UTF-8. Can be set to NULL.
///
/// Default: NULL
#[unsafe(no_mangle)]
pub extern "C" fn resvg_options_set_stylesheet(opt: *mut resvg_options, content: *const c_char) {
    ffi_try("resvg_options_set_stylesheet", (), || {
        let Some(opt) = cast_opt(opt) else {
            return;
        };

        if content.is_null() {
            opt.style_sheet = None;
        } else {
            opt.style_sheet = cstr_to_str(content).map(Into::into);
        }
    })
}

/// @brief Sets the default font family.
///
/// Will be used when no `font-family` attribute is set in the SVG.
///
/// Must be UTF-8. NULL is not allowed.
///
/// Default: Times New Roman
#[unsafe(no_mangle)]
pub extern "C" fn resvg_options_set_font_family(opt: *mut resvg_options, family: *const c_char) {
    ffi_try("resvg_options_set_font_family", (), || {
        let Some(opt) = cast_opt(opt) else {
            return;
        };

        let Some(family) = cstr_to_str(family) else {
            log::error!("resvg_options_set_font_family received invalid family");
            return;
        };

        opt.font_family = family.to_string();
    })
}

/// @brief Sets the default font size.
///
/// Will be used when no `font-size` attribute is set in the SVG.
///
/// Default: 12
#[unsafe(no_mangle)]
pub extern "C" fn resvg_options_set_font_size(opt: *mut resvg_options, size: f32) {
    ffi_try("resvg_options_set_font_size", (), || {
        let Some(opt) = cast_opt(opt) else {
            return;
        };

        opt.font_size = size;
    })
}

/// @brief Sets the `serif` font family.
///
/// Must be UTF-8. NULL is not allowed.
///
/// Has no effect when the `text` feature is not enabled.
///
/// Default: Times New Roman
#[unsafe(no_mangle)]
#[allow(unused_variables)]
pub extern "C" fn resvg_options_set_serif_family(opt: *mut resvg_options, family: *const c_char) {
    ffi_try("resvg_options_set_serif_family", (), || {
        #[cfg(feature = "text")]
        {
            let Some(opt) = cast_opt(opt) else {
                return;
            };

            let Some(family) = cstr_to_str(family) else {
                log::error!("resvg_options_set_serif_family received invalid family");
                return;
            };

            opt.fontdb_mut().set_serif_family(family.to_string());
        }
    })
}

/// @brief Sets the `sans-serif` font family.
///
/// Must be UTF-8. NULL is not allowed.
///
/// Has no effect when the `text` feature is not enabled.
///
/// Default: Arial
#[unsafe(no_mangle)]
#[allow(unused_variables)]
pub extern "C" fn resvg_options_set_sans_serif_family(
    opt: *mut resvg_options,
    family: *const c_char,
) {
    ffi_try("resvg_options_set_sans_serif_family", (), || {
        #[cfg(feature = "text")]
        {
            let Some(opt) = cast_opt(opt) else {
                return;
            };

            let Some(family) = cstr_to_str(family) else {
                log::error!("resvg_options_set_sans_serif_family received invalid family");
                return;
            };

            opt.fontdb_mut().set_sans_serif_family(family.to_string());
        }
    })
}

/// @brief Sets the `cursive` font family.
///
/// Must be UTF-8. NULL is not allowed.
///
/// Has no effect when the `text` feature is not enabled.
///
/// Default: Comic Sans MS
#[unsafe(no_mangle)]
#[allow(unused_variables)]
pub extern "C" fn resvg_options_set_cursive_family(opt: *mut resvg_options, family: *const c_char) {
    ffi_try("resvg_options_set_cursive_family", (), || {
        #[cfg(feature = "text")]
        {
            let Some(opt) = cast_opt(opt) else {
                return;
            };

            let Some(family) = cstr_to_str(family) else {
                log::error!("resvg_options_set_cursive_family received invalid family");
                return;
            };

            opt.fontdb_mut().set_cursive_family(family.to_string());
        }
    })
}

/// @brief Sets the `fantasy` font family.
///
/// Must be UTF-8. NULL is not allowed.
///
/// Has no effect when the `text` feature is not enabled.
///
/// Default: Papyrus on macOS, Impact on other OS'es
#[unsafe(no_mangle)]
#[allow(unused_variables)]
pub extern "C" fn resvg_options_set_fantasy_family(opt: *mut resvg_options, family: *const c_char) {
    ffi_try("resvg_options_set_fantasy_family", (), || {
        #[cfg(feature = "text")]
        {
            let Some(opt) = cast_opt(opt) else {
                return;
            };

            let Some(family) = cstr_to_str(family) else {
                log::error!("resvg_options_set_fantasy_family received invalid family");
                return;
            };

            opt.fontdb_mut().set_fantasy_family(family.to_string());
        }
    })
}

/// @brief Sets the `monospace` font family.
///
/// Must be UTF-8. NULL is not allowed.
///
/// Has no effect when the `text` feature is not enabled.
///
/// Default: Courier New
#[unsafe(no_mangle)]
#[allow(unused_variables)]
pub extern "C" fn resvg_options_set_monospace_family(
    opt: *mut resvg_options,
    family: *const c_char,
) {
    ffi_try("resvg_options_set_monospace_family", (), || {
        #[cfg(feature = "text")]
        {
            let Some(opt) = cast_opt(opt) else {
                return;
            };

            let Some(family) = cstr_to_str(family) else {
                log::error!("resvg_options_set_monospace_family received invalid family");
                return;
            };

            opt.fontdb_mut().set_monospace_family(family.to_string());
        }
    })
}

/// @brief Sets a comma-separated list of languages.
///
/// Will be used to resolve a `systemLanguage` conditional attribute.
///
/// Example: en,en-US.
///
/// Must be UTF-8. Can be NULL.
///
/// Default: en
#[unsafe(no_mangle)]
pub extern "C" fn resvg_options_set_languages(opt: *mut resvg_options, languages: *const c_char) {
    ffi_try("resvg_options_set_languages", (), || {
        let Some(opt) = cast_opt(opt) else {
            return;
        };

        if languages.is_null() {
            opt.languages = Vec::new();
            return;
        }

        let Some(languages_str) = cstr_to_str(languages) else {
            log::error!("resvg_options_set_languages received invalid UTF-8");
            return;
        };

        let mut values = Vec::new();
        for lang in languages_str.split(',') {
            values.push(lang.trim().to_string());
        }

        opt.languages = values;
    })
}

/// @brief A shape rendering method.
#[repr(C)]
#[allow(missing_docs)]
#[derive(Copy, Clone)]
pub enum resvg_shape_rendering {
    OPTIMIZE_SPEED,
    CRISP_EDGES,
    GEOMETRIC_PRECISION,
}

/// @brief Sets the default shape rendering method.
///
/// Will be used when an SVG element's `shape-rendering` property is set to `auto`.
///
/// Default: `RESVG_SHAPE_RENDERING_GEOMETRIC_PRECISION`
#[unsafe(no_mangle)]
pub extern "C" fn resvg_options_set_shape_rendering_mode(
    opt: *mut resvg_options,
    mode: resvg_shape_rendering,
) {
    ffi_try("resvg_options_set_shape_rendering_mode", (), || {
        let Some(opt) = cast_opt(opt) else {
            return;
        };

        opt.shape_rendering = match mode as i32 {
            0 => usvg::ShapeRendering::OptimizeSpeed,
            1 => usvg::ShapeRendering::CrispEdges,
            2 => usvg::ShapeRendering::GeometricPrecision,
            _ => {
                log::warn!("resvg_options_set_shape_rendering_mode received invalid mode");
                return;
            }
        };
    })
}

/// @brief A text rendering method.
#[repr(C)]
#[allow(missing_docs)]
#[derive(Copy, Clone)]
pub enum resvg_text_rendering {
    OPTIMIZE_SPEED,
    OPTIMIZE_LEGIBILITY,
    GEOMETRIC_PRECISION,
}

/// @brief Sets the default text rendering method.
///
/// Will be used when an SVG element's `text-rendering` property is set to `auto`.
///
/// Default: `RESVG_TEXT_RENDERING_OPTIMIZE_LEGIBILITY`
#[unsafe(no_mangle)]
pub extern "C" fn resvg_options_set_text_rendering_mode(
    opt: *mut resvg_options,
    mode: resvg_text_rendering,
) {
    ffi_try("resvg_options_set_text_rendering_mode", (), || {
        let Some(opt) = cast_opt(opt) else {
            return;
        };

        opt.text_rendering = match mode as i32 {
            0 => usvg::TextRendering::OptimizeSpeed,
            1 => usvg::TextRendering::OptimizeLegibility,
            2 => usvg::TextRendering::GeometricPrecision,
            _ => {
                log::warn!("resvg_options_set_text_rendering_mode received invalid mode");
                return;
            }
        };
    })
}

/// @brief A image rendering method.
#[repr(C)]
#[allow(missing_docs)]
#[derive(Copy, Clone)]
pub enum resvg_image_rendering {
    OPTIMIZE_QUALITY,
    OPTIMIZE_SPEED,
}

/// @brief Sets the default image rendering method.
///
/// Will be used when an SVG element's `image-rendering` property is set to `auto`.
///
/// Default: `RESVG_IMAGE_RENDERING_OPTIMIZE_QUALITY`
#[unsafe(no_mangle)]
pub extern "C" fn resvg_options_set_image_rendering_mode(
    opt: *mut resvg_options,
    mode: resvg_image_rendering,
) {
    ffi_try("resvg_options_set_image_rendering_mode", (), || {
        let Some(opt) = cast_opt(opt) else {
            return;
        };

        opt.image_rendering = match mode as i32 {
            0 => usvg::ImageRendering::OptimizeQuality,
            1 => usvg::ImageRendering::OptimizeSpeed,
            _ => {
                log::warn!("resvg_options_set_image_rendering_mode received invalid mode");
                return;
            }
        };
    })
}

/// @brief Loads a font data into the internal fonts database.
///
/// Prints a warning into the log when the data is not a valid TrueType font.
///
/// Has no effect when the `text` feature is not enabled.
#[unsafe(no_mangle)]
#[allow(unused_variables)]
pub extern "C" fn resvg_options_load_font_data(
    opt: *mut resvg_options,
    data: *const c_char,
    len: usize,
) {
    ffi_try("resvg_options_load_font_data", (), || {
        #[cfg(feature = "text")]
        {
            let Some(opt) = cast_opt(opt) else {
                return;
            };

            if data.is_null() {
                log::error!("resvg_options_load_font_data received null data");
                return;
            }

            let data = unsafe { slice::from_raw_parts(data as *const u8, len) };
            opt.fontdb_mut().load_font_data(data.to_vec());
        }
    })
}

/// @brief Loads a font file into the internal fonts database.
///
/// Prints a warning into the log when the data is not a valid TrueType font.
///
/// Has no effect when the `text` feature is not enabled.
///
/// @return #resvg_error with RESVG_OK, RESVG_ERROR_NOT_AN_UTF8_STR or RESVG_ERROR_FILE_OPEN_FAILED
#[unsafe(no_mangle)]
#[allow(unused_variables)]
pub extern "C" fn resvg_options_load_font_file(
    opt: *mut resvg_options,
    file_path: *const c_char,
) -> i32 {
    ffi_try(
        "resvg_options_load_font_file",
        resvg_error::PARSING_FAILED as i32,
        || {
            #[cfg(feature = "text")]
            {
                let Some(opt) = cast_opt(opt) else {
                    return resvg_error::PARSING_FAILED as i32;
                };

                let file_path = match cstr_to_str(file_path) {
                    Some(v) => v,
                    None => return resvg_error::NOT_AN_UTF8_STR as i32,
                };

                if opt.fontdb_mut().load_font_file(file_path).is_ok() {
                    resvg_error::OK as i32
                } else {
                    log::error!("failed to load font file '{}'", file_path);
                    resvg_error::FILE_OPEN_FAILED as i32
                }
            }

            #[cfg(not(feature = "text"))]
            {
                resvg_error::OK as i32
            }
        },
    )
}

/// @brief Loads system fonts into the internal fonts database.
///
/// This method is very IO intensive.
///
/// This method should be executed only once per #resvg_options.
///
/// The system scanning is not perfect, so some fonts may be omitted.
/// Please send a bug report in this case.
///
/// Prints warnings into the log.
///
/// Has no effect when the `text` feature is not enabled.
#[unsafe(no_mangle)]
#[allow(unused_variables)]
pub extern "C" fn resvg_options_load_system_fonts(opt: *mut resvg_options) {
    ffi_try("resvg_options_load_system_fonts", (), || {
        #[cfg(feature = "text")]
        {
            let Some(opt) = cast_opt(opt) else {
                return;
            };

            opt.fontdb_mut().load_system_fonts();
        }
    })
}

/// @brief Destroys the #resvg_options.
#[unsafe(no_mangle)]
pub extern "C" fn resvg_options_destroy(opt: *mut resvg_options) {
    ffi_try("resvg_options_destroy", (), || {
        if opt.is_null() {
            log::warn!("resvg_options_destroy called with null");
            return;
        }

        unsafe {
            let _ = Box::from_raw(opt);
        }
    })
}

// TODO: use resvg::Tree
/// @brief An opaque pointer to the rendering tree.
pub struct resvg_render_tree(pub usvg::Tree);

/// @brief Creates #resvg_render_tree from file.
///
/// .svg and .svgz files are supported.
///
/// See #resvg_is_image_empty for details.
///
/// @param file_path UTF-8 file path.
/// @param opt Rendering options. Must not be NULL.
/// @param tree Parsed render tree. Should be destroyed via #resvg_tree_destroy.
/// @return #resvg_error
#[unsafe(no_mangle)]
pub extern "C" fn resvg_parse_tree_from_file(
    file_path: *const c_char,
    opt: *const resvg_options,
    tree: *mut *mut resvg_render_tree,
) -> i32 {
    ffi_try(
        "resvg_parse_tree_from_file",
        resvg_error::PARSING_FAILED as i32,
        || {
            let Some(file_path) = cstr_to_str(file_path) else {
                return resvg_error::NOT_AN_UTF8_STR as i32;
            };

            if opt.is_null() {
                log::error!("resvg_parse_tree_from_file received null opt");
                return resvg_error::PARSING_FAILED as i32;
            }

            if tree.is_null() {
                log::error!("resvg_parse_tree_from_file received null tree out pointer");
                return resvg_error::PARSING_FAILED as i32;
            }

            let raw_opt = unsafe { &*opt };

            let file_data = match std::fs::read(file_path) {
                Ok(data) => data,
                Err(err) => {
                    log::error!("failed to read '{}': {}", file_path, err);
                    return resvg_error::FILE_OPEN_FAILED as i32;
                }
            };

            let utree = match usvg::Tree::from_data(&file_data, &raw_opt.options) {
                Ok(tree_value) => tree_value,
                Err(err) => {
                    log::error!("failed to parse '{}': {:?}", file_path, err);
                    return convert_error(err) as i32;
                }
            };

            let tree_box = Box::new(resvg_render_tree(utree));
            unsafe {
                *tree = Box::into_raw(tree_box);
            }

            resvg_error::OK as i32
        },
    )
}

/// @brief Creates #resvg_render_tree from data.
///
/// See #resvg_is_image_empty for details.
///
/// @param data SVG data. Can contain SVG string or gzip compressed data. Must not be NULL.
/// @param len Data length.
/// @param opt Rendering options. Must not be NULL.
/// @param tree Parsed render tree. Should be destroyed via #resvg_tree_destroy.
/// @return #resvg_error
#[unsafe(no_mangle)]
pub extern "C" fn resvg_parse_tree_from_data(
    data: *const c_char,
    len: usize,
    opt: *const resvg_options,
    tree: *mut *mut resvg_render_tree,
) -> i32 {
    ffi_try(
        "resvg_parse_tree_from_data",
        resvg_error::PARSING_FAILED as i32,
        || {
            if data.is_null() {
                log::error!("resvg_parse_tree_from_data received null data");
                return resvg_error::PARSING_FAILED as i32;
            }

            if opt.is_null() {
                log::error!("resvg_parse_tree_from_data received null opt");
                return resvg_error::PARSING_FAILED as i32;
            }

            if tree.is_null() {
                log::error!("resvg_parse_tree_from_data received null tree out pointer");
                return resvg_error::PARSING_FAILED as i32;
            }

            let data = unsafe { slice::from_raw_parts(data as *const u8, len) };
            let raw_opt = unsafe { &*opt };

            let utree = match usvg::Tree::from_data(data, &raw_opt.options) {
                Ok(tree_value) => tree_value,
                Err(err) => {
                    log::error!("failed to parse SVG data: {:?}", err);
                    return convert_error(err) as i32;
                }
            };

            let tree_box = Box::new(resvg_render_tree(utree));
            unsafe {
                *tree = Box::into_raw(tree_box);
            }

            resvg_error::OK as i32
        },
    )
}

/// @brief Checks that tree has any nodes.
///
/// @param tree Render tree.
/// @return Returns `true` if tree has no nodes.
#[unsafe(no_mangle)]
pub extern "C" fn resvg_is_image_empty(tree: *const resvg_render_tree) -> bool {
    ffi_try("resvg_is_image_empty", true, || {
        let Some(tree) = cast_tree(tree) else {
            return true;
        };

        !tree.0.root().has_children()
    })
}

/// @brief Returns an image size.
///
/// The size of an image that is required to render this SVG.
///
/// Note that elements outside the viewbox will be clipped. This is by design.
/// If you want to render the whole SVG content, use #resvg_get_image_bbox instead.
///
/// @param tree Render tree.
/// @return Image size.
#[unsafe(no_mangle)]
pub extern "C" fn resvg_get_image_size(tree: *const resvg_render_tree) -> resvg_size {
    ffi_try(
        "resvg_get_image_size",
        resvg_size {
            width: 0.0,
            height: 0.0,
        },
        || {
            let Some(tree) = cast_tree(tree) else {
                return resvg_size {
                    width: 0.0,
                    height: 0.0,
                };
            };

            let size = tree.0.size();

            resvg_size {
                width: size.width(),
                height: size.height(),
            }
        },
    )
}

/// @brief Returns an object bounding box.
///
/// This bounding box does not include objects stroke and filter regions.
/// This is what SVG calls "absolute object bonding box".
///
/// If you're looking for a "complete" bounding box see #resvg_get_image_bbox
///
/// @param tree Render tree.
/// @param bbox Image's object bounding box.
/// @return `false` if an image has no elements.
#[unsafe(no_mangle)]
pub extern "C" fn resvg_get_object_bbox(
    tree: *const resvg_render_tree,
    bbox: *mut resvg_rect,
) -> bool {
    ffi_try("resvg_get_object_bbox", false, || {
        let Some(tree) = cast_tree(tree) else {
            return false;
        };

        if bbox.is_null() {
            log::error!("resvg_get_object_bbox received null bbox");
            return false;
        }

        if let Some(r) = tree.0.root().abs_bounding_box().to_non_zero_rect() {
            unsafe {
                *bbox = resvg_rect {
                    x: r.x(),
                    y: r.y(),
                    width: r.width(),
                    height: r.height(),
                };
            }

            true
        } else {
            false
        }
    })
}

/// @brief Returns an image bounding box.
///
/// This bounding box contains the maximum SVG dimensions.
/// It's size can be bigger or smaller than #resvg_get_image_size
/// Use it when you want to avoid clipping of elements that are outside the SVG viewbox.
///
/// @param tree Render tree.
/// @param bbox Image's bounding box.
/// @return `false` if an image has no elements.
#[unsafe(no_mangle)]
pub extern "C" fn resvg_get_image_bbox(
    tree: *const resvg_render_tree,
    bbox: *mut resvg_rect,
) -> bool {
    ffi_try("resvg_get_image_bbox", false, || {
        let Some(tree) = cast_tree(tree) else {
            return false;
        };

        if bbox.is_null() {
            log::error!("resvg_get_image_bbox received null bbox");
            return false;
        }

        // `abs_layer_bounding_box` returns 0x0x1x1 for empty groups, so we need additional checks.
        if tree.0.root().has_children() || !tree.0.root().filters().is_empty() {
            let r = tree.0.root().abs_layer_bounding_box();
            unsafe {
                *bbox = resvg_rect {
                    x: r.x(),
                    y: r.y(),
                    width: r.width(),
                    height: r.height(),
                };
            }

            true
        } else {
            false
        }
    })
}

/// @brief Returns `true` if a renderable node with such an ID exists.
///
/// @param tree Render tree.
/// @param id Node's ID. UTF-8 string. Must not be NULL.
/// @return `true` if a node exists.
/// @return `false` if a node doesn't exist or ID isn't a UTF-8 string.
/// @return `false` if a node exists, but not renderable.
#[unsafe(no_mangle)]
pub extern "C" fn resvg_node_exists(tree: *const resvg_render_tree, id: *const c_char) -> bool {
    ffi_try("resvg_node_exists", false, || {
        let Some(id) = cstr_to_str(id) else {
            log::warn!("Provided ID is not a UTF-8 string.");
            return false;
        };

        let Some(tree) = cast_tree(tree) else {
            return false;
        };

        tree.0.node_by_id(id).is_some()
    })
}

/// @brief Returns node's transform by ID.
///
/// @param tree Render tree.
/// @param id Node's ID. UTF-8 string. Must not be NULL.
/// @param transform Node's transform.
/// @return `true` if a node exists.
/// @return `false` if a node doesn't exist or ID isn't a UTF-8 string.
/// @return `false` if a node exists, but not renderable.
#[unsafe(no_mangle)]
pub extern "C" fn resvg_get_node_transform(
    tree: *const resvg_render_tree,
    id: *const c_char,
    transform: *mut resvg_transform,
) -> bool {
    ffi_try("resvg_get_node_transform", false, || {
        let Some(id) = cstr_to_str(id) else {
            log::warn!("Provided ID is not a UTF-8 string.");
            return false;
        };

        if transform.is_null() {
            log::error!("resvg_get_node_transform received null transform");
            return false;
        }

        let Some(tree) = cast_tree(tree) else {
            return false;
        };

        if let Some(node) = tree.0.node_by_id(id) {
            let abs_ts = node.abs_transform();

            unsafe {
                *transform = resvg_transform {
                    a: abs_ts.sx,
                    b: abs_ts.ky,
                    c: abs_ts.kx,
                    d: abs_ts.sy,
                    e: abs_ts.tx,
                    f: abs_ts.ty,
                };
            }

            true
        } else {
            false
        }
    })
}

/// @brief Returns node's bounding box in canvas coordinates by ID.
///
/// @param tree Render tree.
/// @param id Node's ID. Must not be NULL.
/// @param bbox Node's bounding box.
/// @return `false` if a node with such an ID does not exist
/// @return `false` if ID isn't a UTF-8 string.
/// @return `false` if ID is an empty string
#[unsafe(no_mangle)]
pub extern "C" fn resvg_get_node_bbox(
    tree: *const resvg_render_tree,
    id: *const c_char,
    bbox: *mut resvg_rect,
) -> bool {
    ffi_try("resvg_get_node_bbox", false, || {
        get_node_bbox(tree, id, bbox, &|node| node.abs_bounding_box())
    })
}

/// @brief Returns node's bounding box, including stroke, in canvas coordinates by ID.
///
/// @param tree Render tree.
/// @param id Node's ID. Must not be NULL.
/// @param bbox Node's bounding box.
/// @return `false` if a node with such an ID does not exist
/// @return `false` if ID isn't a UTF-8 string.
/// @return `false` if ID is an empty string
#[unsafe(no_mangle)]
pub extern "C" fn resvg_get_node_stroke_bbox(
    tree: *const resvg_render_tree,
    id: *const c_char,
    bbox: *mut resvg_rect,
) -> bool {
    ffi_try("resvg_get_node_stroke_bbox", false, || {
        get_node_bbox(tree, id, bbox, &|node| node.abs_stroke_bounding_box())
    })
}

fn get_node_bbox(
    tree: *const resvg_render_tree,
    id: *const c_char,
    bbox: *mut resvg_rect,
    f: &dyn Fn(&usvg::Node) -> usvg::Rect,
) -> bool {
    let Some(id) = cstr_to_str(id) else {
        log::warn!("Provided ID is not a UTF-8 string.");
        return false;
    };

    if id.is_empty() {
        log::warn!("Node ID must not be empty.");
        return false;
    }

    let Some(tree) = cast_tree(tree) else {
        return false;
    };

    if bbox.is_null() {
        log::error!("get_node_bbox received null bbox");
        return false;
    }

    match tree.0.node_by_id(id) {
        Some(node) => {
            let r = f(node);
            unsafe {
                *bbox = resvg_rect {
                    x: r.x(),
                    y: r.y(),
                    width: r.width(),
                    height: r.height(),
                };
            }

            true
        }
        None => {
            log::warn!("No node with '{}' ID is in the tree.", id);
            false
        }
    }
}

/// @brief Destroys the #resvg_render_tree.
#[unsafe(no_mangle)]
pub extern "C" fn resvg_tree_destroy(tree: *mut resvg_render_tree) {
    ffi_try("resvg_tree_destroy", (), || {
        if tree.is_null() {
            log::warn!("resvg_tree_destroy called with null");
            return;
        }

        unsafe {
            let _ = Box::from_raw(tree);
        }
    })
}

fn convert_error(e: usvg::Error) -> resvg_error {
    match e {
        usvg::Error::NotAnUtf8Str => resvg_error::NOT_AN_UTF8_STR,
        usvg::Error::SvgzFeatureNotEnabled => resvg_error::SVGZ_UNSUPPORTED,
        usvg::Error::MalformedGZip => resvg_error::MALFORMED_GZIP,
        usvg::Error::ElementsLimitReached => resvg_error::ELEMENTS_LIMIT_REACHED,
        usvg::Error::InvalidSize => resvg_error::INVALID_SIZE,
        usvg::Error::ParsingFailed(_) => resvg_error::PARSING_FAILED,
    }
}

/// @brief Renders the #resvg_render_tree onto the pixmap.
///
/// @param tree A render tree.
/// @param transform A root SVG transform. Can be used to position SVG inside the `pixmap`.
/// @param width Pixmap width.
/// @param height Pixmap height.
/// @param pixmap Pixmap data. Should have width*height*4 size and contain
///               premultiplied RGBA8888 pixels.
#[unsafe(no_mangle)]
pub extern "C" fn resvg_render(
    tree: *const resvg_render_tree,
    transform: resvg_transform,
    width: u32,
    height: u32,
    pixmap: *mut c_char,
) {
    ffi_try("resvg_render", (), || {
        let Some(tree) = cast_tree(tree) else {
            return;
        };

        if pixmap.is_null() {
            log::error!("resvg_render received null pixmap");
            return;
        }

        let pixmap_len = width as usize * height as usize * tiny_skia::BYTES_PER_PIXEL;
        let pixmap: &mut [u8] = unsafe { std::slice::from_raw_parts_mut(pixmap as *mut u8, pixmap_len) };

        let Some(mut pixmap) = tiny_skia::PixmapMut::from_bytes(pixmap, width, height) else {
            log::error!(
                "resvg_render failed to create pixmap from buffer, width={}, height={}",
                width,
                height
            );
            return;
        };

        resvg::render(&tree.0, transform.to_tiny_skia(), &mut pixmap);
    })
}

/// @brief Renders a Node by ID onto the image.
///
/// @param tree A render tree.
/// @param id Node's ID. Must not be NULL.
/// @param transform A root SVG transform. Can be used to position SVG inside the `pixmap`.
/// @param width Pixmap width.
/// @param height Pixmap height.
/// @param pixmap Pixmap data. Should have width*height*4 size and contain
///               premultiplied RGBA8888 pixels.
/// @return `false` when `id` is not a non-empty UTF-8 string.
/// @return `false` when the selected `id` is not present.
/// @return `false` when an element has a zero bbox.
#[unsafe(no_mangle)]
pub extern "C" fn resvg_render_node(
    tree: *const resvg_render_tree,
    id: *const c_char,
    transform: resvg_transform,
    width: u32,
    height: u32,
    pixmap: *mut c_char,
) -> bool {
    ffi_try("resvg_render_node", false, || {
        let Some(tree) = cast_tree(tree) else {
            return false;
        };

        let Some(id) = cstr_to_str(id) else {
            return false;
        };

        if id.is_empty() {
            log::warn!("Node with an empty ID cannot be rendered.");
            return false;
        }

        if pixmap.is_null() {
            log::error!("resvg_render_node received null pixmap");
            return false;
        }

        if let Some(node) = tree.0.node_by_id(id) {
            let pixmap_len = width as usize * height as usize * tiny_skia::BYTES_PER_PIXEL;
            let pixmap: &mut [u8] =
                unsafe { std::slice::from_raw_parts_mut(pixmap as *mut u8, pixmap_len) };

            let Some(mut pixmap) = tiny_skia::PixmapMut::from_bytes(pixmap, width, height) else {
                log::error!(
                    "resvg_render_node failed to create pixmap from buffer, width={}, height={}",
                    width,
                    height
                );
                return false;
            };

            resvg::render_node(node, transform.to_tiny_skia(), &mut pixmap).is_some()
        } else {
            log::warn!("A node with '{}' ID wasn't found.", id);
            false
        }
    })
}

/// A simple stderr logger.
static LOGGER: SimpleLogger = SimpleLogger;

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::LevelFilter::Error
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let target = if !record.target().is_empty() {
                record.target()
            } else {
                record.module_path().unwrap_or_default()
            };

            let line = record.line().unwrap_or(0);
            let args = record.args();

            match record.level() {
                log::Level::Error => eprintln!("Error (in {}:{}): {}", target, line, args),
                log::Level::Warn => eprintln!("Warning (in {}:{}): {}", target, line, args),
                log::Level::Info => eprintln!("Info (in {}:{}): {}", target, line, args),
                log::Level::Debug => eprintln!("Debug (in {}:{}): {}", target, line, args),
                log::Level::Trace => eprintln!("Trace (in {}:{}): {}", target, line, args),
            }
        }
    }

    fn flush(&self) {}
}