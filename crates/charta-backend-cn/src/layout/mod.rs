//! Layout flow shared by the official-document templates.
//!
//! `header` builds 版头, `body` builds 主体, `record` builds 版记. `text`
//! owns run construction and style-intent adoption; `footer` owns page
//! furniture. Shared metadata and content helpers live here.

pub(crate) mod body;
pub(crate) mod cover;
pub(crate) mod footer;
pub(crate) mod header;
pub(crate) mod minutes;
pub(crate) mod record;
pub(crate) mod text;
pub(crate) mod toc;

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

pub(crate) fn chinese_date_numerals(value: &str) -> String {
    match parse_iso_date(value) {
        Some((year, month, day)) => format!(
            "{}年{}月{}日",
            chinese_year(year),
            crate::numbering::chinese_number(month),
            crate::numbering::chinese_number(day)
        ),
        None => value.to_owned(),
    }
}

fn chinese_year(year: i32) -> String {
    const DIGITS: [&str; 10] = ["〇", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
    year.to_string()
        .chars()
        .filter_map(|ch| ch.to_digit(10).map(|digit| DIGITS[digit as usize]))
        .collect()
}
