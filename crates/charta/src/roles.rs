//! Structure roles and the backend role registry.
//!
//! Base roles are fixed by the engine. A document may declare additional roles
//! under the engine-control metadata namespace `_charta.roles`; a backend may
//! register the roles it understands through [`Backend::roles`](crate::Backend).

use crate::ir::{Block, BlockKind, Document, MetaValue, Metadata, RoleDeclaration, SourceSpan};
use crate::report::{Issue, IssueDetails, Severity};
use std::collections::{BTreeMap, BTreeSet};

pub const ENGINE_METADATA_KEY: &str = "_charta";

pub const BASE_ROLES: &[&str] = &[
    "heading", "body", "list", "table", "image", "emphasis", "quote",
];

#[derive(Debug, Clone, Default)]
pub struct RoleRegistry {
    known: BTreeMap<String, Option<String>>,
}

impl RoleRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();
        for role in BASE_ROLES {
            registry
                .known
                .insert((*role).to_owned(), Some("基础结构角色".to_owned()));
        }
        registry
    }

    pub fn register(&mut self, declaration: RoleDeclaration) {
        self.known
            .entry(declaration.name)
            .or_insert(declaration.description);
    }

    pub fn knows(&self, name: &str) -> bool {
        self.known.contains_key(name)
    }

    pub fn is_base(name: &str) -> bool {
        BASE_ROLES.contains(&name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.known.keys().map(String::as_str)
    }
}

/// Read `_charta.roles` from document metadata. This is engine-control
/// metadata, not a backend metadata field, and is therefore not subject to
/// backend schema validation.
pub fn declarations_from_metadata(metadata: &Metadata) -> (Vec<RoleDeclaration>, Vec<Issue>) {
    let mut declarations = Vec::new();
    let mut issues = Vec::new();
    let Some(control) = metadata.get(ENGINE_METADATA_KEY) else {
        return (declarations, issues);
    };
    let Some(control) = control.as_map() else {
        issues.push(control_issue(
            "`_charta` 必须是映射",
            "map",
            control.type_name(),
        ));
        return (declarations, issues);
    };
    let Some(roles) = control.get("roles") else {
        return (declarations, issues);
    };
    let Some(items) = roles.as_list() else {
        issues.push(control_issue(
            "`_charta.roles` 必须是列表",
            "list",
            roles.type_name(),
        ));
        return (declarations, issues);
    };

    let mut seen = BTreeSet::new();
    for (index, item) in items.iter().enumerate() {
        let declaration = match item {
            MetaValue::String(name) => Some(RoleDeclaration {
                name: name.clone(),
                description: None,
            }),
            MetaValue::Map(map) => {
                let name = map.get("name").and_then(MetaValue::as_str);
                let description = map
                    .get("description")
                    .and_then(MetaValue::as_str)
                    .map(str::to_owned);
                match name {
                    Some(name) => Some(RoleDeclaration {
                        name: name.to_owned(),
                        description,
                    }),
                    None => {
                        issues.push(control_issue(
                            format!("`_charta.roles` 第 {} 项缺少字符串 `name`", index + 1),
                            "string `name`",
                            "缺失",
                        ));
                        None
                    }
                }
            }
            other => {
                issues.push(control_issue(
                    format!("`_charta.roles` 第 {} 项必须是字符串或映射", index + 1),
                    "string | map",
                    other.type_name(),
                ));
                None
            }
        };

        if let Some(declaration) = declaration {
            if !is_valid_role_name(&declaration.name) {
                issues.push(control_issue(
                    format!("扩展角色名 `{}` 不合法", declaration.name),
                    "仅允许 ASCII 字母、数字、下划线和连字符",
                    &declaration.name,
                ));
                continue;
            }
            if !seen.insert(declaration.name.clone()) {
                issues.push(control_issue(
                    format!("扩展角色 `{}` 重复声明", declaration.name),
                    "唯一的角色名",
                    &declaration.name,
                ));
                continue;
            }
            declarations.push(declaration);
        }
    }
    (declarations, issues)
}

pub fn validate_roles(document: &Document, registry: &RoleRegistry) -> Vec<Issue> {
    let mut issues = Vec::new();
    let declared: BTreeSet<&str> = document
        .declared_roles
        .iter()
        .map(|role| role.name.as_str())
        .collect();

    for declaration in &document.declared_roles {
        if RoleRegistry::is_base(&declaration.name) {
            issues.push(Issue::schema(
                format!("基础角色 `{}` 不能被重新声明", declaration.name),
                "_charta.roles",
                "扩展角色名（非基础角色）",
                &declaration.name,
                None,
            ));
        } else if !registry.knows(&declaration.name) {
            issues.push(Issue::new(
                Severity::Warning,
                format!("后端未注册扩展角色 `{}`", declaration.name),
                None,
                IssueDetails::RenderWarning {
                    code: "unregistered_role".to_owned(),
                    context: Some(format!(
                        "角色 `{}` 已声明，但后端将其按透明容器处理",
                        declaration.name
                    )),
                },
            ));
        }
    }

    let mut usages = Vec::new();
    collect_extended_roles(&document.blocks, &mut usages);
    for (role, source) in usages {
        if !declared.contains(role.as_str()) {
            issues.push(Issue::schema(
                format!("扩展角色 `{role}` 未在文档元数据中声明"),
                "_charta.roles",
                "包含该角色的声明",
                role.clone(),
                Some(source),
            ));
        }
    }

    issues
}

fn collect_extended_roles(blocks: &[Block], out: &mut Vec<(String, SourceSpan)>) {
    for block in blocks {
        match &block.kind {
            BlockKind::Quote { blocks } => collect_extended_roles(blocks, out),
            BlockKind::Extended { role, blocks } => {
                out.push((role.clone(), block.source.clone()));
                collect_extended_roles(blocks, out);
            }
            _ => {}
        }
    }
}

fn is_valid_role_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn control_issue(
    message: impl Into<String>,
    expected: impl Into<String>,
    actual: impl Into<String>,
) -> Issue {
    Issue::schema(message, "_charta.roles", expected, actual, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{FrontendId, MetaValue};

    #[test]
    fn parses_role_declarations() {
        let mut metadata = Metadata::new();
        let mut control = BTreeMap::new();
        control.insert(
            "roles".to_owned(),
            MetaValue::List(vec![
                MetaValue::String("appendix".to_owned()),
                MetaValue::Map(BTreeMap::from([(
                    "name".to_owned(),
                    MetaValue::String("notes".to_owned()),
                )])),
            ]),
        );
        metadata.insert("_charta", MetaValue::Map(control));
        let (declarations, issues) = declarations_from_metadata(&metadata);
        assert_eq!(declarations.len(), 2);
        assert!(issues.is_empty());
    }

    #[test]
    fn rejects_undeclared_extended_roles() {
        let document = Document::new(FrontendId::Markdown, Metadata::new()).with_blocks(vec![
            Block::extended(
                "appendix",
                vec![Block::paragraph(
                    Vec::new(),
                    SourceSpan::point("a.md", 1, 1, 0),
                )],
                SourceSpan::point("a.md", 1, 1, 0),
            ),
        ]);
        let issues = validate_roles(&document, &RoleRegistry::new());
        assert_eq!(issues.len(), 1);
        assert!(issues[0].is_blocking());
    }
}
