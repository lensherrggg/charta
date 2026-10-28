//! Numbering strategy for GB/T 9704.
//!
//! Lists use static text ordinals at four levels; the fifth level falls back
//! to body paragraphs. Headings opt in through FR-IR-5 and use native Word
//! multilevel numbering (`一、` / `（一）` / `1.` / `（1）`); style data without a
//! numbering definition falls back to static ordinals. Image captions use
//! static `图 N` labels.

use charta::ListBlock;

/// Static heading counters used only when the style data has no native
/// numbering definition.
#[derive(Debug, Default)]
pub(crate) struct HeadingNumbers {
    counters: [u32; 4],
    reported_static: bool,
    reported_flattened: bool,
}

#[derive(Debug, Default)]
pub(crate) struct NumberingState {
    pub(crate) headings: HeadingNumbers,
    pub(crate) figure: u32,
    pub(crate) heading_num_id: Option<u32>,
    pub(crate) heading_instances: u32,
    pub(crate) heading_word_reported: bool,
    pub(crate) heading_flattened_reported: bool,
}

#[derive(Debug, Default)]
pub(crate) struct HeadingMarker {
    pub(crate) text: Option<String>,
    pub(crate) report_static: bool,
    pub(crate) report_flattened: bool,
}

impl HeadingNumbers {
    pub(crate) fn reset(&mut self) {
        self.counters = [0; 4];
    }

    pub(crate) fn marker(&mut self, level: u8) -> HeadingMarker {
        let report_static = !self.reported_static;
        self.reported_static = true;

        if !(1..=4).contains(&level) {
            let report_flattened = !self.reported_flattened;
            self.reported_flattened = true;
            return HeadingMarker {
                text: None,
                report_static,
                report_flattened,
            };
        }

        let index = (level - 1) as usize;
        self.counters[index] += 1;
        for deeper in &mut self.counters[index + 1..] {
            *deeper = 0;
        }
        let number = self.counters[index];
        let text = match level {
            1 => format!("{}、", chinese_number(number)),
            2 => format!("（{}）", chinese_number(number)),
            3 => format!("{number}. "),
            _ => format!("（{number}）"),
        };
        HeadingMarker {
            text: Some(text),
            report_static,
            report_flattened: false,
        }
    }
}

fn chinese_number(number: u32) -> String {
    const DIGITS: [&str; 10] = ["零", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
    match number {
        0 => "零".to_owned(),
        1..=9 => DIGITS[number as usize].to_owned(),
        10 => "十".to_owned(),
        11..=19 => format!("十{}", DIGITS[(number % 10) as usize]),
        20..=99 => {
            let tens = number / 10;
            let ones = number % 10;
            if ones == 0 {
                format!("{}十", DIGITS[tens as usize])
            } else {
                format!("{}十{}", DIGITS[tens as usize], DIGITS[ones as usize])
            }
        }
        _ => number.to_string(),
    }
}

pub(crate) fn list_marker(depth: u32, number: u32) -> Option<String> {
    match depth {
        0 => Some(format!("（{number}）")),
        1 => Some(match circled_number(number) {
            Some(symbol) => symbol.to_string(),
            None => format!("({number})"),
        }),
        2 => Some(format!("{}.", letter_label(number))),
        3 => Some(format!("{}.", letter_label(number).to_lowercase())),
        _ => None,
    }
}

fn circled_number(number: u32) -> Option<char> {
    let code = match number {
        1..=20 => 0x2460 + number - 1,
        21..=35 => 0x3251 + number - 21,
        36..=50 => 0x32B1 + number - 36,
        _ => return None,
    };
    char::from_u32(code)
}

fn letter_label(number: u32) -> String {
    if number == 0 {
        return "0".to_owned();
    }
    let mut remaining = number;
    let mut label = String::new();
    while remaining > 0 {
        let offset = ((remaining - 1) % 26) as u8;
        label.insert(0, (b'A' + offset) as char);
        remaining = (remaining - 1) / 26;
    }
    label
}

pub(crate) fn figure_label(number: u32) -> String {
    format!("图 {number}\u{3000}")
}

pub(crate) fn max_list_depth(list: &ListBlock, depth: u32) -> u32 {
    list.items
        .iter()
        .flat_map(|item| item.children.iter())
        .map(|child| max_list_depth(child, depth + 1))
        .max()
        .unwrap_or(depth)
}
