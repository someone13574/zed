use anyhow::Result;
use std::ops::Range;

pub fn migrate_keymap(_text: &str) -> Result<Option<String>> {
    Ok(None)
}

pub fn migrate_settings(_text: &str) -> Result<Option<String>> {
    Ok(None)
}

pub fn migrate_edit_prediction_provider_settings(_text: &str) -> Result<Option<String>> {
    Ok(None)
}

pub type MigrationPatterns = &'static [(&'static str, fn(&str, &(), &()) -> Option<(Range<usize>, String)>)];
