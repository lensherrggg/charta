//! Structured reports.
//!
//! The report schema is frozen for the first major series: fields may be added,
//! existing field names and shapes may not change. Every issue carries a
//! severity so callers can decide whether to continue.

use crate::ir::SourceSpan;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

pub const REPORT_VERSION: &str = "1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// The conversion must not be treated as a valid result.
    Blocking,
    /// The conversion completed, but the caller should review the issue.
    Warning,
}

impl Severity {
    pub fn is_blocking(self) -> bool {
        matches!(self, Self::Blocking)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blocking => "blocking",
            Self::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    SchemaValidation,
    StyleOverridden,
    NonDeterministic,
    UnsupportedSyntax,
    RenderWarning,
}

impl IssueKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SchemaValidation => "schema_validation",
            Self::StyleOverridden => "style_overridden",
            Self::NonDeterministic => "non_deterministic",
            Self::UnsupportedSyntax => "unsupported_syntax",
            Self::RenderWarning => "render_warning",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::SchemaValidation => "元数据校验",
            Self::StyleOverridden => "样式意图被覆盖",
            Self::NonDeterministic => "不可静态确定",
            Self::UnsupportedSyntax => "契约外语法",
            Self::RenderWarning => "渲染告警",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub severity: Severity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<SourceSpan>,
    #[serde(flatten)]
    pub details: IssueDetails,
}

impl Issue {
    pub fn new(
        severity: Severity,
        message: impl Into<String>,
        location: Option<SourceSpan>,
        details: IssueDetails,
    ) -> Self {
        Self {
            severity,
            message: message.into(),
            location,
            details,
        }
    }

    pub fn kind(&self) -> IssueKind {
        self.details.kind()
    }

    pub fn is_blocking(&self) -> bool {
        self.severity.is_blocking()
    }

    pub fn schema(
        message: impl Into<String>,
        field: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
        location: Option<SourceSpan>,
    ) -> Self {
        Self::new(
            Severity::Blocking,
            message,
            location,
            IssueDetails::SchemaValidation {
                field: field.into(),
                expected: expected.into(),
                actual: actual.into(),
            },
        )
    }

    pub fn style_overridden(
        message: impl Into<String>,
        property: impl Into<String>,
        intent: impl Into<String>,
        adopted: impl Into<String>,
        reason: impl Into<String>,
        location: Option<SourceSpan>,
    ) -> Self {
        Self::new(
            Severity::Warning,
            message,
            location,
            IssueDetails::StyleOverridden {
                property: property.into(),
                intent: intent.into(),
                adopted: adopted.into(),
                reason: reason.into(),
            },
        )
    }

    pub fn non_deterministic(
        message: impl Into<String>,
        reason: impl Into<String>,
        impact: impl Into<String>,
        location: Option<SourceSpan>,
    ) -> Self {
        Self::new(
            Severity::Warning,
            message,
            location,
            IssueDetails::NonDeterministic {
                reason: reason.into(),
                impact: impact.into(),
            },
        )
    }

    pub fn unsupported(
        message: impl Into<String>,
        snippet: impl Into<String>,
        handling: impl Into<String>,
        location: Option<SourceSpan>,
    ) -> Self {
        Self::new(
            Severity::Warning,
            message,
            location,
            IssueDetails::UnsupportedSyntax {
                snippet: snippet.into(),
                handling: handling.into(),
            },
        )
    }

    pub fn render_warning(
        message: impl Into<String>,
        code: impl Into<String>,
        context: Option<String>,
        location: Option<SourceSpan>,
    ) -> Self {
        Self::new(
            Severity::Warning,
            message,
            location,
            IssueDetails::RenderWarning {
                code: code.into(),
                context,
            },
        )
    }

    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IssueDetails {
    SchemaValidation {
        field: String,
        expected: String,
        actual: String,
    },
    StyleOverridden {
        property: String,
        intent: String,
        adopted: String,
        reason: String,
    },
    NonDeterministic {
        reason: String,
        impact: String,
    },
    UnsupportedSyntax {
        snippet: String,
        handling: String,
    },
    RenderWarning {
        code: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<String>,
    },
}

impl IssueDetails {
    pub fn kind(&self) -> IssueKind {
        match self {
            Self::SchemaValidation { .. } => IssueKind::SchemaValidation,
            Self::StyleOverridden { .. } => IssueKind::StyleOverridden,
            Self::NonDeterministic { .. } => IssueKind::NonDeterministic,
            Self::UnsupportedSyntax { .. } => IssueKind::UnsupportedSyntax,
            Self::RenderWarning { .. } => IssueKind::RenderWarning,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportSummary {
    pub blocking: usize,
    pub warnings: usize,
    pub total: usize,
    pub by_kind: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub report_version: String,
    pub engine_version: String,
    pub source: String,
    pub frontend: String,
    pub backend: String,
    pub summary: ReportSummary,
    pub issues: Vec<Issue>,
}

impl Report {
    pub fn new(
        source: impl Into<String>,
        frontend: impl Into<String>,
        backend: impl Into<String>,
    ) -> Self {
        Self {
            report_version: REPORT_VERSION.to_owned(),
            engine_version: crate::VERSION.to_owned(),
            source: source.into(),
            frontend: frontend.into(),
            backend: backend.into(),
            summary: ReportSummary::default(),
            issues: Vec::new(),
        }
    }

    pub fn add(&mut self, issue: Issue) {
        self.issues.push(issue);
    }

    pub fn extend(&mut self, issues: impl IntoIterator<Item = Issue>) {
        self.issues.extend(issues);
    }

    pub fn add_all(&mut self, issues: Vec<Issue>) {
        self.issues.extend(issues);
    }

    pub fn blocking_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.is_blocking())
            .count()
    }

    pub fn has_blocking(&self) -> bool {
        self.issues.iter().any(Issue::is_blocking)
    }

    /// Sort issues deterministically and recompute the summary.
    pub fn finish(mut self) -> Self {
        self.issues.sort_by(|left, right| {
            let severity = right.is_blocking().cmp(&left.is_blocking());
            severity
                .then_with(|| location_key(&left.location).cmp(&location_key(&right.location)))
                .then_with(|| left.message.cmp(&right.message))
        });
        self.summary = summarize(&self.issues);
        self
    }

    pub fn to_json(&self) -> Result<String> {
        let mut json = serde_json::to_string_pretty(self)?;
        json.push('\n');
        Ok(json)
    }

    pub fn write_json(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path, self.to_json()?)?;
        Ok(())
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Charta 转换报告\n\n");
        let _ = writeln!(out, "- 引擎版本：{}", self.engine_version);
        let _ = writeln!(out, "- 源文件：`{}`", self.source);
        let _ = writeln!(out, "- 前端：{}", self.frontend);
        let _ = writeln!(out, "- 后端：{}", self.backend);
        let _ = writeln!(out, "- 报告格式：{}", self.report_version);
        if self.has_blocking() {
            let _ = writeln!(
                out,
                "- 结论：**失败**，存在 {} 项阻断性错误",
                self.blocking_count()
            );
        } else {
            out.push_str("- 结论：**通过**，无阻断性错误\n");
        }
        let _ = writeln!(
            out,
            "- 问题总数：{}（阻断 {}，告警 {}）\n",
            self.summary.total, self.summary.blocking, self.summary.warnings
        );

        out.push_str("## 摘要\n\n");
        if self.summary.by_kind.is_empty() {
            out.push_str("未发现问题。\n");
            return out;
        }
        out.push_str("| 类型 | 数量 |\n|---|---:|\n");
        for (kind, count) in &self.summary.by_kind {
            let label = issue_kind_from_str(kind)
                .map(IssueKind::label_zh)
                .unwrap_or(kind.as_str());
            let _ = writeln!(out, "| {} | {} |", label, count);
        }
        out.push('\n');

        let blocking: Vec<&Issue> = self
            .issues
            .iter()
            .filter(|issue| issue.is_blocking())
            .collect();
        let warnings: Vec<&Issue> = self
            .issues
            .iter()
            .filter(|issue| !issue.is_blocking())
            .collect();

        if !blocking.is_empty() {
            out.push_str("## 阻断性错误\n\n");
            for (index, issue) in blocking.iter().enumerate() {
                write_issue(&mut out, index + 1, issue);
            }
        }
        if !warnings.is_empty() {
            out.push_str("## 非阻断告警\n\n");
            for (index, issue) in warnings.iter().enumerate() {
                write_issue(&mut out, index + 1, issue);
            }
        }
        out
    }

    pub fn write_markdown(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path, self.to_markdown())?;
        Ok(())
    }
}

fn summarize(issues: &[Issue]) -> ReportSummary {
    let mut summary = ReportSummary {
        total: issues.len(),
        ..ReportSummary::default()
    };
    for issue in issues {
        if issue.is_blocking() {
            summary.blocking += 1;
        } else {
            summary.warnings += 1;
        }
        *summary
            .by_kind
            .entry(issue.kind().as_str().to_owned())
            .or_insert(0) += 1;
    }
    summary
}

fn location_key(location: &Option<SourceSpan>) -> (String, usize, usize) {
    match location {
        Some(span) => (span.file.clone(), span.line, span.column),
        None => (String::new(), 0, 0),
    }
}

fn issue_kind_from_str(value: &str) -> Option<IssueKind> {
    Some(match value {
        "schema_validation" => IssueKind::SchemaValidation,
        "style_overridden" => IssueKind::StyleOverridden,
        "non_deterministic" => IssueKind::NonDeterministic,
        "unsupported_syntax" => IssueKind::UnsupportedSyntax,
        "render_warning" => IssueKind::RenderWarning,
        _ => return None,
    })
}

fn write_issue(out: &mut String, index: usize, issue: &Issue) {
    let _ = writeln!(
        out,
        "### {}. {} — {}\n",
        index,
        issue.kind().label_zh(),
        issue.message
    );
    if let Some(location) = &issue.location {
        let _ = writeln!(out, "- 位置：`{}`", location.display());
    }
    match &issue.details {
        IssueDetails::SchemaValidation {
            field,
            expected,
            actual,
        } => {
            let _ = writeln!(out, "- 字段：`{}`", field);
            let _ = writeln!(out, "- 期望：{}", expected);
            let _ = writeln!(out, "- 实际：{}", actual);
        }
        IssueDetails::StyleOverridden {
            property,
            intent,
            adopted,
            reason,
        } => {
            let _ = writeln!(out, "- 属性：{}", property);
            let _ = writeln!(out, "- 源样式意图：{}", intent);
            let _ = writeln!(out, "- 实际采用：{}", adopted);
            let _ = writeln!(out, "- 原因：{}", reason);
        }
        IssueDetails::NonDeterministic { reason, impact } => {
            let _ = writeln!(out, "- 原因：{}", reason);
            let _ = writeln!(out, "- 影响范围：{}", impact);
        }
        IssueDetails::UnsupportedSyntax { snippet, handling } => {
            let snippet = snippet.replace('`', "'");
            let _ = writeln!(out, "- 语法片段：`{}`", snippet);
            let _ = writeln!(out, "- 处理方式：{}", handling);
        }
        IssueDetails::RenderWarning { code, context } => {
            let _ = writeln!(out, "- 代码：`{}`", code);
            if let Some(context) = context {
                let _ = writeln!(out, "- 上下文：{}", context);
            }
        }
    }
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::SourceSpan;

    #[test]
    fn summary_and_markdown_are_stable() {
        let mut report = Report::new("input.md", "markdown", "gb9704");
        report.add(Issue::unsupported(
            "不支持的语法",
            "[链接](x)",
            "降级为正文",
            Some(SourceSpan::point("input.md", 3, 5, 20)),
        ));
        report.add(Issue::schema(
            "缺少必填字段",
            "title",
            "string（必填）",
            "缺失",
            None,
        ));
        let report = report.finish();
        assert_eq!(report.summary.total, 2);
        assert_eq!(report.summary.blocking, 1);
        assert_eq!(report.summary.warnings, 1);
        assert_eq!(report.blocking_count(), 1);
        let json = report.to_json().unwrap();
        assert!(json.contains("\"report_version\": \"1.0\""));
        assert!(json.contains("\"type\": \"schema_validation\""));
        let markdown = report.to_markdown();
        assert!(markdown.contains("## 阻断性错误"));
        assert!(markdown.contains("input.md:3:5"));
        assert!(markdown.contains("降级为正文"));
    }

    #[test]
    fn deterministic_ordering_puts_blocking_first() {
        let mut report = Report::new("a.md", "markdown", "gb9704");
        report.add(Issue::unsupported(
            "warn",
            "x",
            "skip",
            Some(SourceSpan::point("a.md", 2, 1, 0)),
        ));
        report.add(Issue::schema("block", "x", "y", "z", None));
        let report = report.finish();
        assert_eq!(report.issues[0].severity, Severity::Blocking);
    }
}
