// font-kit/src/source.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A database of installed fonts that can be queried.

use error::SelectionError;
use family::Family;
use family_handle::FamilyHandle;
use family_name::FamilyName;
use font::Font;
use handle::Handle;
use matching::{self, Description};
use properties::Properties;

#[cfg(all(target_os = "macos", not(feature = "source-fontconfig-default")))]
pub use sources::core_text::CoreTextSource as SystemSource;
#[cfg(all(target_family = "windows", not(feature = "source-fontconfig-default")))]
pub use sources::directwrite::DirectWriteSource as SystemSource;
#[cfg(any(not(any(target_os = "android", target_os = "macos", target_family = "windows")),
          feature = "source-fontconfig-default"))]
pub use sources::fontconfig::FontconfigSource as SystemSource;
#[cfg(all(target_os = "android", not(feature = "source-fontconfig-default")))]
pub use sources::fs::FsSource as SystemSource;

// FIXME(pcwalton): These could expand to multiple fonts, and they could be language-specific.
const DEFAULT_FONT_FAMILY_SERIF: &'static str = "Times New Roman";
const DEFAULT_FONT_FAMILY_SANS_SERIF: &'static str = "Arial";
const DEFAULT_FONT_FAMILY_MONOSPACE: &'static str = "Courier New";
const DEFAULT_FONT_FAMILY_CURSIVE: &'static str = "Comic Sans MS";
const DEFAULT_FONT_FAMILY_FANTASY: &'static str = "Papyrus";

/// A database of installed fonts that can be queried.
pub trait Source {
    /// Returns all families installed on the system.
    fn all_families(&self) -> Result<Vec<String>, SelectionError>;

    /// Looks up a font family by name.
    fn select_family_by_name(&self, family_name: &str) -> Result<FamilyHandle, SelectionError>;

    /// Selects a font by PostScript name, which should be a unique identifier.
    ///
    /// The default implementation, which is used by the DirectWrite and the filesystem backends,
    /// does a brute-force search of installed fonts to find the one that matches.
    fn select_by_postscript_name(&self, postscript_name: &str) -> Result<Handle, SelectionError> {
        // TODO(pcwalton): Optimize this by searching for families with similar names first.
        for family_name in try!(self.all_families()) {
            if let Ok(family_handle) = self.select_family_by_name(&family_name) {
                if let Ok(family) = Family::<Font>::from_handle(&family_handle) {
                    for (handle, font) in family_handle.fonts().iter().zip(family.fonts().iter()) {
                        if font.postscript_name() == postscript_name {
                            return Ok((*handle).clone())
                        }
                    }
                }
            }
        }
        Err(SelectionError::NotFound)
    }

    // FIXME(pcwalton): This only returns one family instead of multiple families for the generic
    // family names.
    #[doc(hidden)]
    fn select_family_by_generic_name(&self, family_name: &FamilyName)
                              -> Result<FamilyHandle, SelectionError> {
        match *family_name {
            FamilyName::Title(ref title) => self.select_family_by_name(title),
            FamilyName::Serif => self.select_family_by_name(DEFAULT_FONT_FAMILY_SERIF),
            FamilyName::SansSerif => self.select_family_by_name(DEFAULT_FONT_FAMILY_SANS_SERIF),
            FamilyName::Monospace => self.select_family_by_name(DEFAULT_FONT_FAMILY_MONOSPACE),
            FamilyName::Cursive => self.select_family_by_name(DEFAULT_FONT_FAMILY_CURSIVE),
            FamilyName::Fantasy => self.select_family_by_name(DEFAULT_FONT_FAMILY_FANTASY),
        }
    }

    /// Performs font matching according to the CSS Fonts Level 3 specification and returns the
    /// handle.
    #[inline]
    fn select_best_match(&self, family_names: &[FamilyName], properties: &Properties)
                         -> Result<Handle, SelectionError> {
        for family_name in family_names {
            if let Ok(family_handle) = self.select_family_by_generic_name(family_name) {
                let candidates = try!(self.select_descriptions_in_family(&family_handle));
                if let Ok(index) = matching::find_best_match(&candidates, properties) {
                    return Ok(family_handle.fonts[index].clone())
                }
            }
        }
        Err(SelectionError::NotFound)
    }

    #[doc(hidden)]
    fn select_descriptions_in_family(&self, family: &FamilyHandle)
                                     -> Result<Vec<Description>, SelectionError> {
        let mut fields = vec![];
        for font_handle in family.fonts() {
            let font = Font::from_handle(font_handle).unwrap();
            let (family_name, properties) = (font.family_name(), font.properties());
            fields.push(Description {
                family_name,
                properties,
            })
        }
        Ok(fields)
    }
}
