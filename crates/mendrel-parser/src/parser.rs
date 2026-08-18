use mendrel_diagnostics::{
    Diagnostic, DiagnosticFix, DiagnosticSpan, MISSING_TOKEN, TextEdit, UNSUPPORTED_SYNTAX,
};
use mendrel_source::{ByteSpan, SourceFile};
use mendrel_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxTree, Token, TokenKind};

use crate::lexer::{LexResult, lex};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseResult {
    pub tree: SyntaxTree,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse(source: &SourceFile) -> ParseResult {
    let LexResult {
        tokens,
        diagnostics,
    } = lex(source);
    Parser {
        source,
        tokens,
        cursor: 0,
        diagnostics,
    }
    .parse_source_file()
}

struct Parser<'source> {
    source: &'source SourceFile,
    tokens: Vec<Token>,
    cursor: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn parse_source_file(mut self) -> ParseResult {
        let mut children = Vec::new();
        children.push(SyntaxElement::Node(self.parse_module_decl()));

        while !self.at_eof() {
            if self.at_function_start() {
                children.push(SyntaxElement::Node(self.parse_function_decl()));
            } else {
                children.push(SyntaxElement::Node(self.recover_top_level()));
            }
        }
        self.take_trivia(&mut children);
        self.bump(&mut children);

        ParseResult {
            tree: SyntaxTree::new(SyntaxNode::new(SyntaxKind::SourceFile, children)),
            diagnostics: self.diagnostics,
        }
    }

    fn parse_module_decl(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        self.expect_text("module", &mut children);
        children.push(SyntaxElement::Node(self.parse_qualified_name()));
        self.expect_text(";", &mut children);
        SyntaxNode::new(SyntaxKind::ModuleDecl, children)
    }

    fn parse_function_decl(&mut self) -> SyntaxNode {
        let children = vec![
            SyntaxElement::Node(self.parse_function_head()),
            SyntaxElement::Node(self.parse_block()),
        ];
        SyntaxNode::new(SyntaxKind::FunctionDecl, children)
    }

    fn parse_function_head(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        self.expect_text("pub", &mut children);
        self.expect_text("fn", &mut children);
        children.push(SyntaxElement::Node(self.parse_identifier()));
        self.expect_text("(", &mut children);
        if self.at_text(")") {
            children.push(SyntaxElement::Node(self.reject_empty_region(
                "the addition-only Phase 1 slice requires at least one typed parameter",
            )));
        } else if !self.at_eof() {
            children.push(SyntaxElement::Node(self.parse_parameter()));
            while self.at_text(",") {
                if self.significant_token_is_followed_by(")") {
                    children.push(SyntaxElement::Node(self.reject_current_token(
                        "trailing parameter commas are outside the addition-only Phase 1 slice",
                    )));
                    break;
                }
                self.bump_expected(&mut children);
                children.push(SyntaxElement::Node(self.parse_parameter()));
            }
        }
        self.expect_text(")", &mut children);
        if self.at_text("->") {
            self.bump_expected(&mut children);
            children.push(SyntaxElement::Node(self.parse_type()));
        } else {
            self.insert_missing("return type annotation", &mut children);
        }
        SyntaxNode::new(SyntaxKind::FunctionHead, children)
    }

    fn parse_parameter(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        if self.at_text("move") {
            children.push(SyntaxElement::Node(self.reject_current_token(
                "parameter modifiers are outside the implemented Phase 1 subset",
            )));
        }
        children.push(SyntaxElement::Node(self.parse_identifier()));
        self.expect_text(":", &mut children);
        children.push(SyntaxElement::Node(self.parse_type()));
        SyntaxNode::new(SyntaxKind::Parameter, children)
    }

    fn parse_type(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        if self.at_identifier() || self.at_text("Self") {
            children.push(SyntaxElement::Node(self.parse_qualified_name()));
        } else {
            self.insert_missing("type", &mut children);
        }
        SyntaxNode::new(SyntaxKind::Type, children)
    }

    fn parse_qualified_name(&mut self) -> SyntaxNode {
        let mut children = vec![SyntaxElement::Node(self.parse_identifier())];
        while self.at_text(".") {
            self.bump_expected(&mut children);
            children.push(SyntaxElement::Node(self.parse_identifier()));
        }
        SyntaxNode::new(SyntaxKind::QualifiedName, children)
    }

    fn parse_identifier(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        if self.at_identifier() {
            self.bump_expected(&mut children);
        } else {
            self.insert_missing("identifier", &mut children);
        }
        SyntaxNode::new(SyntaxKind::Identifier, children)
    }

    fn parse_block(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        self.expect_text("{", &mut children);
        if self.at_text("}") {
            children.push(SyntaxElement::Node(self.reject_empty_region(
                "the addition-only Phase 1 slice requires a trailing expression",
            )));
        } else if !self.at_eof() {
            if self.at_expression_start() {
                children.push(SyntaxElement::Node(self.parse_expression()));
            } else {
                children.push(SyntaxElement::Node(self.recover_block_tail()));
            }
        }
        if !self.at_text("}") && !self.at_eof() {
            children.push(SyntaxElement::Node(self.recover_block_tail()));
        }
        self.expect_text("}", &mut children);
        SyntaxNode::new(SyntaxKind::Block, children)
    }

    fn parse_expression(&mut self) -> SyntaxNode {
        SyntaxNode::new(
            SyntaxKind::Expression,
            vec![SyntaxElement::Node(self.parse_additive_expression())],
        )
    }

    fn parse_additive_expression(&mut self) -> SyntaxNode {
        let mut children = vec![SyntaxElement::Node(self.parse_multiplicative_expression())];
        while self.at_text("+") {
            self.bump_expected(&mut children);
            children.push(SyntaxElement::Node(self.parse_multiplicative_expression()));
        }
        SyntaxNode::new(SyntaxKind::AdditiveExpression, children)
    }

    fn parse_multiplicative_expression(&mut self) -> SyntaxNode {
        SyntaxNode::new(
            SyntaxKind::MultiplicativeExpression,
            vec![SyntaxElement::Node(self.parse_unary_expression())],
        )
    }

    fn parse_unary_expression(&mut self) -> SyntaxNode {
        SyntaxNode::new(
            SyntaxKind::UnaryExpression,
            vec![SyntaxElement::Node(self.parse_postfix_expression())],
        )
    }

    fn parse_postfix_expression(&mut self) -> SyntaxNode {
        SyntaxNode::new(
            SyntaxKind::PostfixExpression,
            vec![SyntaxElement::Node(self.parse_primary_expression())],
        )
    }

    fn parse_primary_expression(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        if self.at_identifier() {
            children.push(SyntaxElement::Node(SyntaxNode::new(
                SyntaxKind::PathExpression,
                vec![SyntaxElement::Node(SyntaxNode::new(
                    SyntaxKind::QualifiedName,
                    vec![SyntaxElement::Node(self.parse_identifier())],
                ))],
            )));
        } else {
            self.insert_missing("expression", &mut children);
        }
        SyntaxNode::new(SyntaxKind::PrimaryExpression, children)
    }

    fn recover_top_level(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        let significant = self
            .top_level_error_token()
            .expect("top-level recovery is not called at EOF");
        let span = significant.span;
        let actual = significant.text.clone();
        self.push_diagnostic(
            &UNSUPPORTED_SYNTAX,
            span,
            format!("`{actual}` is outside the implemented Phase 1 subset"),
            None,
            Some(actual),
        );

        self.take_trivia(&mut children);
        let mut brace_depth = 0_u32;
        let mut consumed = false;
        while !self.at_eof() {
            if consumed && brace_depth == 0 && self.at_function_start() {
                break;
            }
            let token = self.tokens[self.cursor].clone();
            if token.text == "{" {
                brace_depth += 1;
            } else if token.text == "}" && brace_depth > 0 {
                brace_depth -= 1;
            }
            consumed |= !token.kind.is_trivia();
            self.bump(&mut children);
            if token.text == ";" && brace_depth == 0 {
                break;
            }
        }
        SyntaxNode::new(SyntaxKind::Error, children)
    }

    fn recover_block_tail(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        let significant = self
            .current_significant()
            .expect("block recovery is not called at EOF");
        if significant.kind != TokenKind::Invalid {
            let span = significant.span;
            let actual = significant.text.clone();
            self.push_diagnostic(
                &UNSUPPORTED_SYNTAX,
                span,
                "block contains syntax outside the implemented Phase 1 subset".to_owned(),
                None,
                Some(actual),
            );
        }

        let mut nested_brace_depth = 0_u32;
        while !self.at_eof() {
            if nested_brace_depth == 0 && self.at_text("}") {
                break;
            }
            let token = self.tokens[self.cursor].clone();
            if !token.kind.is_trivia() {
                if token.text == "{" {
                    nested_brace_depth += 1;
                } else if token.text == "}" {
                    nested_brace_depth = nested_brace_depth.saturating_sub(1);
                }
            }
            self.bump(&mut children);
        }
        SyntaxNode::new(SyntaxKind::Error, children)
    }

    fn reject_current_token(&mut self, summary: &str) -> SyntaxNode {
        let mut children = Vec::new();
        let significant = self
            .current_significant()
            .expect("rejected token exists")
            .clone();
        self.push_diagnostic(
            &UNSUPPORTED_SYNTAX,
            significant.span,
            summary.to_owned(),
            None,
            Some(significant.text),
        );
        self.bump_expected(&mut children);
        SyntaxNode::new(SyntaxKind::Error, children)
    }

    fn reject_empty_region(&mut self, summary: &str) -> SyntaxNode {
        let significant = self
            .current_significant()
            .expect("rejected empty region has a following token")
            .clone();
        self.push_diagnostic(
            &UNSUPPORTED_SYNTAX,
            significant.span,
            summary.to_owned(),
            None,
            Some(significant.text),
        );
        SyntaxNode::new(SyntaxKind::Error, Vec::new())
    }

    fn at_function_start(&self) -> bool {
        let index = self.significant_index();
        if self
            .tokens
            .get(index)
            .is_none_or(|token| token.text != "pub")
        {
            return false;
        }
        let index = self.next_significant_index(index + 1);
        self.tokens
            .get(index)
            .is_some_and(|token| token.text == "fn")
    }

    fn top_level_error_token(&self) -> Option<&Token> {
        let current = self.significant_index();
        let token = self.tokens.get(current)?;
        if token.text == "pub" {
            let next = self.next_significant_index(current + 1);
            if let Some(modifier) = self.tokens.get(next).filter(|candidate| {
                matches!(candidate.text.as_str(), "internal" | "async" | "unsafe")
            }) {
                return Some(modifier);
            }
        }
        Some(token)
    }

    fn at_expression_start(&self) -> bool {
        self.at_identifier()
    }

    fn significant_token_is_followed_by(&self, expected: &str) -> bool {
        let current = self.significant_index();
        let next = self.next_significant_index(current + 1);
        self.tokens
            .get(next)
            .is_some_and(|token| token.text == expected)
    }

    fn at_identifier(&self) -> bool {
        self.current_significant()
            .is_some_and(|token| token.kind == TokenKind::Identifier)
    }

    fn at_text(&self, expected: &str) -> bool {
        self.current_significant()
            .is_some_and(|token| token.text == expected)
    }

    fn at_eof(&self) -> bool {
        self.current_significant()
            .is_none_or(|token| token.kind == TokenKind::Eof)
    }

    fn expect_text(&mut self, expected: &str, children: &mut Vec<SyntaxElement>) {
        if self.at_text(expected) {
            self.bump_expected(children);
        } else {
            self.insert_missing(expected, children);
        }
    }

    fn insert_missing(&mut self, expected: &str, children: &mut Vec<SyntaxElement>) {
        let position = self
            .tokens
            .get(self.cursor)
            .map_or(self.source.byte_len(), |token| token.span.start);
        let actual = self
            .current_significant()
            .map_or("end of file", |token| token.text.as_str())
            .to_owned();
        let span = ByteSpan::new(position, position).expect("missing token span is empty");
        children.push(SyntaxElement::Token(Token::new(
            TokenKind::Missing,
            "",
            span,
        )));
        let primary_span = DiagnosticSpan::from_source(self.source, span)
            .expect("missing token spans are source boundaries");
        let diagnostic_index = self.diagnostics.len() + 1;
        let mut diagnostic =
            Diagnostic::from_catalog(&MISSING_TOKEN, primary_span.clone(), self.source.revision())
                .with_id(format!("diag-{diagnostic_index:04}"))
                .with_summary(format!("expected {expected}"))
                .with_expected(expected)
                .with_actual(actual);
        if let Some(replacement) = insertion_replacement(expected) {
            diagnostic = diagnostic.with_fix(DiagnosticFix::machine_applicable(
                format!("fix-{diagnostic_index:04}"),
                format!("Insert `{replacement}`"),
                TextEdit::new(primary_span, replacement, self.source.revision()),
            ));
        }
        self.diagnostics.push(diagnostic);
    }

    fn push_diagnostic(
        &mut self,
        catalog: &'static mendrel_diagnostics::CatalogEntry,
        span: ByteSpan,
        summary: String,
        expected: Option<String>,
        actual: Option<String>,
    ) {
        let mut diagnostic = Diagnostic::from_catalog(
            catalog,
            DiagnosticSpan::from_source(self.source, span)
                .expect("parser diagnostic spans are source boundaries"),
            self.source.revision(),
        )
        .with_id(format!("diag-{:04}", self.diagnostics.len() + 1))
        .with_summary(summary);
        if let Some(expected) = expected {
            diagnostic = diagnostic.with_expected(expected);
        }
        if let Some(actual) = actual {
            diagnostic = diagnostic.with_actual(actual);
        }
        self.diagnostics.push(diagnostic);
    }

    fn bump_expected(&mut self, children: &mut Vec<SyntaxElement>) {
        self.take_trivia(children);
        self.bump(children);
    }

    fn take_trivia(&mut self, children: &mut Vec<SyntaxElement>) {
        while self
            .tokens
            .get(self.cursor)
            .is_some_and(|token| token.kind.is_trivia())
        {
            self.bump(children);
        }
    }

    fn bump(&mut self, children: &mut Vec<SyntaxElement>) {
        if let Some(token) = self.tokens.get(self.cursor).cloned() {
            children.push(SyntaxElement::Token(token));
            self.cursor += 1;
        }
    }

    fn current_significant(&self) -> Option<&Token> {
        self.tokens.get(self.significant_index())
    }

    fn significant_index(&self) -> usize {
        self.next_significant_index(self.cursor)
    }

    fn next_significant_index(&self, mut index: usize) -> usize {
        while self
            .tokens
            .get(index)
            .is_some_and(|token| token.kind.is_trivia())
        {
            index += 1;
        }
        index
    }
}

fn insertion_replacement(expected: &str) -> Option<&str> {
    match expected {
        ";" | "}" | "{" | "(" | ")" | ":" | "," | "->" => Some(expected),
        _ => None,
    }
}
