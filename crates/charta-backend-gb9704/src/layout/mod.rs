//! GB/T 9704 layout flow.
//!
//! `header` builds 版头, `body` builds 主体, `record` builds 版记. `text`
//! owns run construction and style-intent adoption; `footer` owns page
//! furniture. Shared metadata and content helpers live here.

pub(crate) mod body;
pub(crate) mod footer;
pub(crate) mod header;
pub(crate) mod record;
pub(crate) mod text;

use charta::schema::parse_iso_date;
use charta::{MetaValue, Metadata};

pub(crate) fn string_field<'a>(metadata: &'a Metadata, key: &str) -> Option<&'a str> {
    metadata.get(key).and_then(MetaValue::as_str)
}

pub(crate) fn list_field(metadata: &Metadata, key: &str) -> Option<Vec<String>> {
    metadata.get(key).and_then(|value| {
        value.as_list().map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
    })
}

pub(crate) fn with_colon(text: &str) -> String {
    if text.ends_with('：') || text.ends_with(':') {
        text.to_owned()
    } else {
        format!("{text}：")
    }
}

pub(crate) fn with_parentheses(text: &str) -> String {
    if text.starts_with('（') && text.ends_with('）') {
        text.to_owned()
    } else {
        format!("（{text}）")
    }
}

pub(crate) fn chinese_date(value: &str) -> String {
    match parse_iso_date(value) {
        Some((year, month, day)) => format!("{year}年{month}月{day}日"),
        None => value.to_owned(),
    }
}
