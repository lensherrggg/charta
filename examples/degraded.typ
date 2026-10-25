#let charta-meta = (
  title: "Typst 不可静态确定示例",
  doc_number: "演示〔2026〕1号",
  issuer: "演示单位",
  date: "2026-10-25",
  recipient: "各有关单位",
)

= 一、动态内容

#for item in items [循环体中的固定文字]

#while enabled [循环二中的固定文字]

#if enabled [条件成立时的固定文字] else [条件不成立时的文字]

= 二、求值类规则

#set text(font: "SimSun", size: 14pt)

#show heading: it => it

#import "lib.typ": *

= 三、显式样式意图

#text(font: "Arial", size: 20pt, fill: blue)[这一段带有字体、字号和颜色意图。]

按默认配置，颜色意图会被后端采纳，字体与字号意图会被覆盖并写入报告。
