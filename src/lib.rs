use std::collections::BTreeMap;

use syn::{Expr, ExprLit, ExprMethodCall, Lit, Macro, Token, visit::Visit};
use tfm::plugin::types::{Diagnostic, Message, Occurrence, Position, Range};

wit_bindgen::generate!({
    path: "wit",
    world: "analyzer",
    generate_all,
    async: true,
});

struct RustAnalyzer;

impl Guest for RustAnalyzer {
    async fn manifest() -> Manifest {
        Manifest {
            name: "tfm-rust".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            languages: vec!["rust".into()],
        }
    }

    async fn analyze(document: Document) -> Result<Analysis, String> {
        extract(&document.text)
    }
}

export!(RustAnalyzer);

fn extract(source: &str) -> Result<Analysis, String> {
    let file = syn::parse_file(source).map_err(|error| error.to_string())?;
    let mut visitor = MessageVisitor::default();
    visitor.visit_file(&file);
    Ok(Analysis {
        messages: visitor
            .messages
            .into_iter()
            .map(|(source, occurrences)| Message {
                source,
                occurrences,
            })
            .collect(),
        diagnostics: visitor.diagnostics,
    })
}

#[derive(Default)]
struct MessageVisitor {
    messages: BTreeMap<String, Vec<Occurrence>>,
    diagnostics: Vec<Diagnostic>,
    scopes: Vec<String>,
    macro_counts: BTreeMap<String, u32>,
}

impl<'ast> Visit<'ast> for MessageVisitor {
    fn visit_macro(&mut self, macro_call: &'ast Macro) {
        if macro_call.path.is_ident("t") {
            let scope = self.scopes.last().map(String::as_str).unwrap_or("<module>");
            let count = self.macro_counts.entry(scope.into()).or_default();
            *count += 1;
            let anchor = format!("{scope}::t!#{count}");
            match first_string_argument(macro_call, anchor) {
                Ok(Some((source, occurrence))) => {
                    self.messages.entry(source).or_default().push(occurrence);
                }
                Ok(None) => self.diagnostics.push(Diagnostic {
                    message: "t! requires an English string literal as its first argument".into(),
                    range: None,
                }),
                Err(message) => self.diagnostics.push(Diagnostic {
                    message,
                    range: None,
                }),
            }
        }
        syn::visit::visit_macro(self, macro_call);
    }

    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        self.scopes.push(function.sig.ident.to_string());
        syn::visit::visit_item_fn(self, function);
        self.scopes.pop();
    }

    fn visit_expr_method_call(&mut self, method_call: &'ast ExprMethodCall) {
        if matches!(method_call.method.to_string().as_str(), "child" | "label")
            && let Some(Expr::Lit(ExprLit {
                lit: Lit::Str(string),
                ..
            })) = method_call.args.first()
            && is_implicit_ui_text(&string.value())
        {
            let scope = self.scopes.last().map(String::as_str).unwrap_or("<module>");
            let start = string.span().start();
            let end = string.span().end();
            self.messages
                .entry(string.value())
                .or_default()
                .push(Occurrence {
                    range: Range {
                        start: Position {
                            line: start.line as u32,
                            column: start.column as u32,
                        },
                        end: Position {
                            line: end.line as u32,
                            column: end.column as u32,
                        },
                    },
                    symbol: Some(scope.into()),
                    anchor: format!(
                        "implicit::{scope}::{}@{}:{}",
                        method_call.method, start.line, start.column
                    ),
                    context_hints: vec![],
                });
        }
        syn::visit::visit_expr_method_call(self, method_call);
    }
}

fn is_implicit_ui_text(value: &str) -> bool {
    !matches!(value, "AI" | "IP" | "JSON" | "KCC" | "MCP" | "YAML")
        && value
            .chars()
            .filter(|character| character.is_alphabetic())
            .count()
            >= 3
}

fn first_string_argument(
    macro_call: &Macro,
    anchor: String,
) -> Result<Option<(String, Occurrence)>, String> {
    let arguments = macro_call
        .parse_body_with(syn::punctuated::Punctuated::<Expr, Token![,]>::parse_terminated)
        .map_err(|error| error.to_string())?;
    let Some(Expr::Lit(ExprLit {
        lit: Lit::Str(string),
        ..
    })) = arguments.first()
    else {
        return Ok(None);
    };
    let start = string.span().start();
    let end = string.span().end();
    Ok(Some((
        string.value(),
        Occurrence {
            range: Range {
                start: Position {
                    line: start.line as u32,
                    column: start.column as u32,
                },
                end: Position {
                    line: end.line as u32,
                    column: end.column as u32,
                },
            },
            symbol: None,
            anchor,
            context_hints: vec![],
        },
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_rust_translation_macros() {
        let analysis = extract("fn main() { let greeting = t!(\"Hello, world!\"); }").unwrap();
        assert_eq!(analysis.messages.len(), 1);
        assert_eq!(analysis.messages[0].source, "Hello, world!");
        assert_eq!(analysis.messages[0].occurrences[0].anchor, "main::t!#1");
    }

    #[test]
    fn reports_nonliteral_messages() {
        let analysis = extract("fn main() { let greeting = t!(message); }").unwrap();
        assert_eq!(analysis.diagnostics.len(), 1);
    }

    #[test]
    fn extracts_implicit_gpui_child_and_label_literals() {
        let analysis =
            extract("fn render() { div().child(\"Settings\").label(\"Save changes\"); }").unwrap();
        assert_eq!(analysis.messages.len(), 2);
        assert!(analysis.messages.iter().all(|message| {
            message.occurrences[0]
                .anchor
                .starts_with("implicit::render::")
        }));
    }

    #[test]
    fn excludes_emoji_and_short_technical_ui_tokens() {
        let analysis = extract(
            "fn render() { div().child(\"📁\").label(\"AI\").label(\"YAML\").child(\"Settings\"); }",
        )
        .unwrap();
        assert_eq!(analysis.messages.len(), 1);
        assert_eq!(analysis.messages[0].source, "Settings");
    }
}
