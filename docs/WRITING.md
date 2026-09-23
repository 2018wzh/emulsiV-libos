# Technical writing policy / 技术写作规范

## English

Use a Simplified Technical English style for user instructions.
Write short sentences in the active voice.
Give each procedural step one action.
Put the condition before an action that depends on it.
Use one term for one concept.
Keep technical identifiers, command names, and code unchanged.
Define specialist terms before they are needed.
Separate instructions, results, warnings, and limitations.
Do not describe intended tests as completed tests.

Aim for at most 20 words in a procedural sentence and 25 in a descriptive sentence.
Treat code, literal errors, technical names, and tables as technical content rather than prose word-count targets.
Use no more than six sentences in a paragraph.
Prefer direct verbs over noun-heavy descriptions.
Do not remove safety conditions to shorten a sentence.

This policy applies STE principles to a software README.
It is not a declaration of formal ASD-STE100 compliance or dictionary certification.
The requested `ste-writing` skill was not present in the MCPX skill inventory used for this revision.
The work therefore uses the documented style rules and public STE-oriented guidance rather than claiming that skill was executed.

## Chinese

中文采用对应的简明技术写作方式，而不是声称中文通过英语词典认证。
使用主动、明确的句子，每个操作步骤只表达一个主要动作。
先说明前提，再说明操作，最后说明预期结果。
同一概念保持同一术语，代码、命令和技术标识符不翻译。
限制和风险必须保留，不能为了缩短句子而省略安全条件。

两份 README 使用相同章节锚点，并保持代码块完全一致。
`cargo xtask docs` 检查章节、代码块、示例名称和本地链接。
自动检查不等于语言规范的完整认证，最终还需要技术审阅。

## References

[ASD Simplified Technical English](https://www.asd-ste100.org/) explains the controlled-language standard.
[ASD-STE100 skill guidance](https://github.com/danyuchn/asd-ste100-skill/blob/master/SKILL.md) describes a software-documentation adaptation.
