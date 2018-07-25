// font-kit/src/sources/core_text.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A source that contains the installed fonts on macOS.

use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_text::font_collection::{self, CTFontCollection};
use core_text::font_descriptor::{self, CTFontDescriptor};
use core_text::font_manager;
use std::cmp::Ordering;
use std::f32;

use error::SelectionError;
use family_handle::FamilyHandle;
use family_name::FamilyName;
use handle::Handle;
use source::Source;
use properties::{Properties, Stretch, Weight};
use utils;

pub(crate) static FONT_WEIGHT_MAPPING: [f32; 9] =
    [-0.7, -0.5, -0.23, 0.0, 0.2, 0.3, 0.4, 0.6, 0.8];

/// A source that contains the installed fonts on macOS.
pub struct CoreTextSource;

impl CoreTextSource {
    #[inline]
    pub fn new() -> CoreTextSource {
        CoreTextSource
    }

    pub fn all_families(&self) -> Result<Vec<String>, SelectionError> {
        let core_text_family_names = font_manager::copy_available_font_family_names();
        let mut families = Vec::with_capacity(core_text_family_names.len() as usize);
        for core_text_family_name in core_text_family_names.iter() {
            families.push(core_text_family_name.to_string())
        }
        Ok(families)
    }

    pub fn select_family_by_name(&self, family_name: &str)
                                 -> Result<FamilyHandle, SelectionError> {
        let attributes: CFDictionary<CFString, CFType> = CFDictionary::from_CFType_pairs(&[
            (CFString::new("NSFontFamilyAttribute"), CFString::new(family_name).as_CFType()),
        ]);

        let descriptor = font_descriptor::new_from_attributes(&attributes);
        let descriptors = CFArray::from_CFTypes(&[descriptor]);
        let collection = font_collection::new_from_descriptors(&descriptors);
        let handles = create_handles_from_core_text_collection(collection);
        Ok(FamilyHandle::from_font_handles(handles.into_iter()))
    }

    pub fn select_by_postscript_name(&self, postscript_name: &str)
                                     -> Result<Handle, SelectionError> {
        let attributes: CFDictionary<CFString, CFType> = CFDictionary::from_CFType_pairs(&[
            (CFString::new("NSFontNameAttribute"), CFString::new(postscript_name).as_CFType()),
        ]);

        let descriptor = font_descriptor::new_from_attributes(&attributes);
        let descriptors = CFArray::from_CFTypes(&[descriptor]);

        let collection = font_collection::new_from_descriptors(&descriptors);
        let descriptors = collection.get_descriptors();
        if descriptors.len() > 0 {
            Ok(create_handle_from_descriptor(&*descriptors.get(0).unwrap()))
        } else {
            Err(SelectionError::NotFound)
        }
    }

    #[inline]
    pub fn select_best_match(&self, family_names: &[FamilyName], properties: &Properties)
                             -> Result<Handle, SelectionError> {
        <Self as Source>::select_best_match(self, family_names, properties)
    }
}

impl Source for CoreTextSource {
    fn all_families(&self) -> Result<Vec<String>, SelectionError> {
        self.all_families()
    }

    fn select_family_by_name(&self, family_name: &str) -> Result<FamilyHandle, SelectionError> {
        self.select_family_by_name(family_name)
    }

    fn select_by_postscript_name(&self, postscript_name: &str) -> Result<Handle, SelectionError> {
        self.select_by_postscript_name(postscript_name)
    }
}

pub(crate) fn piecewise_linear_lookup(index: f32, mapping: &[f32]) -> f32 {
    let lower_value = mapping[f32::floor(index) as usize];
    let upper_value = mapping[f32::ceil(index) as usize];
    utils::lerp(lower_value, upper_value, f32::fract(index))
}

pub(crate) fn piecewise_linear_find_index(query_value: f32, mapping: &[f32]) -> f32 {
    let upper_index = match mapping.binary_search_by(|value| {
        value.partial_cmp(&query_value).unwrap_or(Ordering::Less)
    }) {
        Ok(index) => return index as f32,
        Err(upper_index) => upper_index,
    };
    if upper_index == 0 {
        return upper_index as f32
    }
    let lower_index = upper_index - 1;
    let (upper_value, lower_value) = (mapping[upper_index], mapping[lower_index]);
    let t = (query_value - lower_value) / (upper_value - lower_value);
    lower_index as f32 + t
}

#[allow(dead_code)]
fn css_to_core_text_font_weight(css_weight: Weight) -> f32 {
    piecewise_linear_lookup(f32::max(100.0, css_weight.0) / 100.0 - 1.0, &FONT_WEIGHT_MAPPING)
}

#[allow(dead_code)]
fn css_stretchiness_to_core_text_width(css_stretchiness: Stretch) -> f32 {
    let css_stretchiness = utils::clamp(css_stretchiness.0, 0.5, 2.0);
    0.25 * piecewise_linear_find_index(css_stretchiness, &Stretch::MAPPING) - 1.0
}

fn create_handles_from_core_text_collection(collection: CTFontCollection) -> Vec<Handle> {
    let mut fonts = vec![];
    let descriptors = collection.get_descriptors();
    for index in 0..descriptors.len() {
        let descriptor = descriptors.get(index).unwrap();
        fonts.push(create_handle_from_descriptor(&*descriptor));
    }
    fonts
}

fn create_handle_from_descriptor(descriptor: &CTFontDescriptor) -> Handle {
    // FIXME(pcwalton): Fish out the font index. Probably we are going to have to open the font…
    Handle::Path {
        path: descriptor.font_path().unwrap(),
        font_index: 0,
    }
}

#[cfg(test)]
mod test {
    use properties::{Stretch, Weight};

    #[test]
    fn test_css_to_core_text_font_weight() {
        // Exact matches
        assert_eq!(super::css_to_core_text_font_weight(Weight(100.0)), -0.7);
        assert_eq!(super::css_to_core_text_font_weight(Weight(400.0)), 0.0);
        assert_eq!(super::css_to_core_text_font_weight(Weight(700.0)), 0.4);
        assert_eq!(super::css_to_core_text_font_weight(Weight(900.0)), 0.8);

        // Linear interpolation
        assert_eq!(super::css_to_core_text_font_weight(Weight(450.0)), 0.1);
    }

    #[test]
    fn test_css_to_core_text_font_stretch() {
        // Exact matches
        assert_eq!(super::css_stretchiness_to_core_text_width(Stretch(1.0)), 0.0);
        assert_eq!(super::css_stretchiness_to_core_text_width(Stretch(0.5)), -1.0);
        assert_eq!(super::css_stretchiness_to_core_text_width(Stretch(2.0)), 1.0);

        // Linear interpolation
        assert_eq!(super::css_stretchiness_to_core_text_width(Stretch(1.7)), 0.85);
    }
}
