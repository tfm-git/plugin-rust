use std::collections::BTreeMap;

use syn::{Expr, ExprLit, Lit, Macro, Token, visit::Visit};
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
}

impl<'ast> Visit<'ast> for MessageVisitor {
    fn visit_macro(&mut self, macro_call: &'ast Macro) {
        if macro_call.path.is_ident("t") {
            match first_string_argument(macro_call) {
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
}

fn first_string_argument(macro_call: &Macro) -> Result<Option<(String, Occurrence)>, String> {
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
    }

    #[test]
    fn reports_nonliteral_messages() {
        let analysis = extract("fn main() { let greeting = t!(message); }").unwrap();
        assert_eq!(analysis.diagnostics.len(), 1);
    }
}
