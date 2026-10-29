//! Metadata schemas for the official-document backends.
//!
//! The engine executes validation; this module only declares the fields.

use charta::{FieldSchema, MetadataSchema};

pub(crate) fn build_schema(header: bool) -> MetadataSchema {
    MetadataSchema::new(vec![
        FieldSchema::string("doc_number", header)
            .describe("发文字号，例如 昆政发〔2026〕1 号；关闭版头时不必填"),
        FieldSchema::string("signer", false)
            .describe("签发人姓名；提供即按上行文版头排版，发文字号居左空一字、签发人居右空一字"),
        FieldSchema::string("issuer", true).describe("发文机关署名"),
        FieldSchema::date("date", true).describe("成文日期（YYYY-MM-DD）"),
        FieldSchema::string("organization", false).describe("发文机关标志，缺省使用 issuer"),
        FieldSchema::string("recipient", false).describe("主送机关，末尾冒号由后端补齐"),
        FieldSchema::string_list("cc", false).describe("抄送机关"),
        FieldSchema::string("issuing_office", false).describe("印发机关，缺省使用 issuer"),
        FieldSchema::date("issuing_date", false).describe("印发日期（YYYY-MM-DD），缺省使用 date"),
        FieldSchema::enumeration("security_level", false, ["绝密", "机密", "秘密"])
            .describe("密级"),
        FieldSchema::enumeration("urgency", false, ["特急", "加急"]).describe("紧急程度"),
        FieldSchema::string("copy_number", false).describe("份号"),
        FieldSchema::string_list("attachments", false).describe("附件名称"),
        FieldSchema::string("note", false).describe("附注，缺省自动加圆括号"),
    ])
}

pub(crate) fn build_general_schema() -> MetadataSchema {
    MetadataSchema::new(vec![
        FieldSchema::string("author", false).describe("姓名；与日期都省略时不生成署名行"),
        FieldSchema::date("date", false).describe("日期（YYYY-MM-DD）；与姓名都省略时不生成署名行"),
        FieldSchema::string("recipient", false).describe("主送机关，末尾冒号由后端补齐"),
    ])
}

pub(crate) fn build_compilation_schema() -> MetadataSchema {
    MetadataSchema::new(vec![
        FieldSchema::string("title", true).describe("封面标题（方正小标宋_GBK 二号）"),
        FieldSchema::date("date", true).describe("封面日期（YYYY-MM-DD）"),
        FieldSchema::enumeration("date_style", false, ["arabic", "chinese"]).describe(
            "封面日期写法：arabic 为 2026年10月25日（默认），chinese 为二〇二六年十月二十五日",
        ),
    ])
}
