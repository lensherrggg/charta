//! Backend-owned metadata schemas.
//!
//! The engine contains no field names or document-standard semantics. It only
//! executes the schema supplied by the target backend and forwards values.

use crate::ir::{MetaValue, Metadata};
use crate::report::Issue;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    String,
    Integer,
    Number,
    Boolean,
    /// ISO calendar date in `YYYY-MM-DD` form.
    Date,
    StringList,
    Any,
}

impl FieldType {
    pub fn expected_description(&self, enum_values: &[String]) -> String {
        let base = match self {
            Self::String => "string",
            Self::Integer => "integer",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Date => "date (YYYY-MM-DD)",
            Self::StringList => "list<string>",
            Self::Any => "any",
        };
        if enum_values.is_empty() {
            base.to_owned()
        } else {
            format!("{}，可选值：{}", base, enum_values.join("、"))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldSchema {
    pub name: String,
    pub field_type: FieldType,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl FieldSchema {
    pub fn new(name: impl Into<String>, field_type: FieldType, required: bool) -> Self {
        Self {
            name: name.into(),
            field_type,
            required,
            enum_values: Vec::new(),
            description: None,
        }
    }

    pub fn string(name: impl Into<String>, required: bool) -> Self {
        Self::new(name, FieldType::String, required)
    }

    pub fn date(name: impl Into<String>, required: bool) -> Self {
        Self::new(name, FieldType::Date, required)
    }

    pub fn string_list(name: impl Into<String>, required: bool) -> Self {
        Self::new(name, FieldType::StringList, required)
    }

    pub fn enumeration(
        name: impl Into<String>,
        required: bool,
        values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut schema = Self::new(name, FieldType::String, required);
        schema.enum_values = values.into_iter().map(Into::into).collect();
        schema
    }

    pub fn describe(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn expected(&self) -> String {
        format!(
            "{}{}",
            self.field_type.expected_description(&self.enum_values),
            if self.required {
                "（必填）"
            } else {
                "（可选）"
            }
        )
    }

    fn validate_value(&self, value: &MetaValue) -> std::result::Result<(), String> {
        match self.field_type {
            FieldType::Any => {}
            FieldType::String => {
                if !matches!(value, MetaValue::String(_)) {
                    return Err(format!("实际类型：{}", value.type_name()));
                }
            }
            FieldType::Integer => {
                if !matches!(value, MetaValue::Integer(_)) {
                    return Err(format!("实际类型：{}", value.type_name()));
                }
            }
            FieldType::Number => {
                if !matches!(value, MetaValue::Integer(_) | MetaValue::Number(_)) {
                    return Err(format!("实际类型：{}", value.type_name()));
                }
            }
            FieldType::Boolean => {
                if !matches!(value, MetaValue::Bool(_)) {
                    return Err(format!("实际类型：{}", value.type_name()));
                }
            }
            FieldType::Date => match value {
                MetaValue::String(text) if parse_iso_date(text).is_some() => {}
                MetaValue::String(text) => {
                    return Err(format!("实际值：{}（不是合法日期）", quote(text)))
                }
                other => return Err(format!("实际类型：{}", other.type_name())),
            },
            FieldType::StringList => match value {
                MetaValue::List(items) => {
                    if let Some((index, item)) = items
                        .iter()
                        .enumerate()
                        .find(|(_, item)| !matches!(item, MetaValue::String(_)))
                    {
                        return Err(format!(
                            "实际类型：list，但第 {} 项是 {}",
                            index + 1,
                            item.type_name()
                        ));
                    }
                }
                other => return Err(format!("实际类型：{}", other.type_name())),
            },
        }

        if !self.enum_values.is_empty() {
            let text = value.as_str().unwrap_or_default();
            if !self.enum_values.iter().any(|candidate| candidate == text) {
                return Err(format!("实际值：{}", quote(text)));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MetadataSchema {
    pub fields: Vec<FieldSchema>,
}

impl MetadataSchema {
    pub fn new(fields: Vec<FieldSchema>) -> Self {
        Self { fields }
    }

    pub fn field(&self, name: &str) -> Option<&FieldSchema> {
        self.fields.iter().find(|field| field.name == name)
    }

    pub fn validate(&self, metadata: &Metadata) -> Vec<Issue> {
        let mut issues = Vec::new();
        for field in &self.fields {
            let value = metadata.get(&field.name);
            match value {
                None | Some(MetaValue::Null) => {
                    if field.required {
                        issues.push(Issue::schema(
                            format!("缺少必填字段 `{}`", field.name),
                            field.name.clone(),
                            field.expected(),
                            "缺失".to_owned(),
                            None,
                        ));
                    }
                }
                Some(value) => {
                    if let Err(actual) = field.validate_value(value) {
                        issues.push(Issue::schema(
                            format!("字段 `{}` 的值不合法", field.name),
                            field.name.clone(),
                            field.expected(),
                            actual,
                            None,
                        ));
                    }
                }
            }
        }
        issues
    }

    pub fn to_json(&self) -> Result<String> {
        let mut json = serde_json::to_string_pretty(self)?;
        json.push('\n');
        Ok(json)
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("| 字段 | 类型 | 必填 | 说明 |\n|---|---|:---:|---|\n");
        for field in &self.fields {
            let description = field.description.clone().unwrap_or_default();
            let _ = writeln!(
                out,
                "| `{}` | `{}` | {} | {} |",
                field.name,
                field.field_type.expected_description(&field.enum_values),
                if field.required { "是" } else { "否" },
                description.replace('|', "\\|")
            );
        }
        out
    }
}

pub fn parse_iso_date(value: &str) -> Option<(i32, u32, u32)> {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year = value.get(0..4)?.parse::<i32>().ok()?;
    let month = value.get(5..7)?.parse::<u32>().ok()?;
    let day = value.get(8..10)?.parse::<u32>().ok()?;
    if !(1..=12).contains(&month) || day == 0 {
        return None;
    }
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return None,
    };
    (day <= max_day).then_some((year, month, day))
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> MetadataSchema {
        MetadataSchema::new(vec![
            FieldSchema::string("title", true).describe("标题"),
            FieldSchema::date("date", true),
            FieldSchema::string_list("cc", false),
            FieldSchema::enumeration("security_level", false, ["绝密", "机密", "秘密"]),
        ])
    }

    #[test]
    fn reports_missing_required_fields() {
        let issues = schema().validate(&Metadata::new());
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().all(Issue::is_blocking));
        assert!(issues[0].message.contains("title"));
    }

    #[test]
    fn reports_type_enum_and_date_errors() {
        let mut metadata = Metadata::new();
        metadata.insert("title", "通知");
        metadata.insert("date", "2026-02-30");
        metadata.insert("cc", MetaValue::List(vec![MetaValue::Integer(1)]));
        metadata.insert("security_level", "内部");
        let issues = schema().validate(&metadata);
        assert_eq!(issues.len(), 3);
        assert!(issues
            .iter()
            .any(|issue| issue.message.contains("`security_level`")));
        assert!(issues.iter().any(|issue| issue.message.contains("`date`")));
    }

    #[test]
    fn accepts_valid_iso_dates() {
        assert!(parse_iso_date("2024-02-29").is_some());
        assert!(parse_iso_date("2026-02-29").is_none());
        assert!(parse_iso_date("2026-13-01").is_none());
        assert!(parse_iso_date("2026-1-01").is_none());
    }

    #[test]
    fn unknown_fields_are_forwarded_without_errors() {
        let mut metadata = Metadata::new();
        metadata.insert("title", "通知");
        metadata.insert("date", "2026-10-25");
        metadata.insert("custom_backend_field", "ok");
        let issues = schema().validate(&metadata);
        assert!(issues.is_empty());
    }
}
