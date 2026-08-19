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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecordFieldContext {
    Record,
    EnumPayload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnumRecoveryContext {
    Body,
    Payload,
}

impl Parser<'_> {
    fn parse_source_file(mut self) -> ParseResult {
        let mut children = Vec::new();
        children.push(SyntaxElement::Node(self.parse_module_decl()));

        while self.at_text("import") {
            children.push(SyntaxElement::Node(self.parse_import_decl()));
        }

        while !self.at_eof() {
            if self.at_enum_start() {
                children.push(SyntaxElement::Node(self.parse_enum_decl()));
            } else if self.at_record_start() {
                children.push(SyntaxElement::Node(self.parse_record_decl()));
            } else if self.at_function_start() {
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

    fn parse_import_decl(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        self.expect_text("import", &mut children);
        children.push(SyntaxElement::Node(self.parse_import_path()));
        if self.at_text("as") {
            self.bump_expected(&mut children);
            children.push(SyntaxElement::Node(self.parse_identifier()));
        }
        self.expect_text(";", &mut children);
        SyntaxNode::new(SyntaxKind::ImportDecl, children)
    }

    fn parse_import_path(&mut self) -> SyntaxNode {
        let mut children = vec![SyntaxElement::Node(self.parse_import_qualified_name())];
        if self.at_text(".") && self.significant_token_is_followed_by("{") {
            self.bump_expected(&mut children);
            self.expect_text("{", &mut children);
            children.push(SyntaxElement::Node(self.parse_import_item()));
            while self.at_text(",") {
                self.bump_expected(&mut children);
                if self.at_text("}") || self.at_text(";") {
                    break;
                }
                children.push(SyntaxElement::Node(self.parse_import_item()));
            }
            self.expect_text("}", &mut children);
        }
        SyntaxNode::new(SyntaxKind::ImportPath, children)
    }

    fn parse_import_qualified_name(&mut self) -> SyntaxNode {
        let mut children = vec![SyntaxElement::Node(self.parse_identifier())];
        while self.at_text(".") && !self.significant_token_is_followed_by("{") {
            self.bump_expected(&mut children);
            children.push(SyntaxElement::Node(self.parse_identifier()));
        }
        SyntaxNode::new(SyntaxKind::QualifiedName, children)
    }

    fn parse_import_item(&mut self) -> SyntaxNode {
        let mut children = vec![SyntaxElement::Node(self.parse_identifier())];
        if self.at_text("as") {
            self.bump_expected(&mut children);
            children.push(SyntaxElement::Node(self.parse_identifier()));
        }
        SyntaxNode::new(SyntaxKind::ImportItem, children)
    }

    fn parse_record_decl(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        if self.at_visibility() {
            children.push(SyntaxElement::Node(self.parse_visibility()));
        }
        self.expect_text("record", &mut children);
        children.push(SyntaxElement::Node(self.parse_identifier()));
        if self.at_text("<") || self.at_text("where") {
            children.push(SyntaxElement::Node(self.recover_top_level()));
            return SyntaxNode::new(SyntaxKind::RecordDecl, children);
        }
        children.push(SyntaxElement::Node(self.parse_record_body()));
        SyntaxNode::new(SyntaxKind::RecordDecl, children)
    }

    fn parse_record_body(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        self.expect_text("{", &mut children);
        while !self.at_text("}") && !self.at_eof() && !self.at_top_level_decl_start() {
            if !self.at_record_field_start() {
                break;
            }
            children.push(SyntaxElement::Node(
                self.parse_record_field(RecordFieldContext::Record),
            ));
        }
        self.expect_text("}", &mut children);
        SyntaxNode::new(SyntaxKind::RecordBody, children)
    }

    fn parse_record_field(&mut self, context: RecordFieldContext) -> SyntaxNode {
        let mut children = Vec::new();
        if self.at_visibility() {
            children.push(SyntaxElement::Node(self.parse_visibility()));
        }
        children.push(SyntaxElement::Node(self.parse_identifier()));
        self.expect_text(":", &mut children);
        if self.at_type_start() {
            children.push(SyntaxElement::Node(self.parse_type()));
            if self.at_unsupported_record_field_type_suffix(context) {
                children.push(SyntaxElement::Node(
                    self.recover_unsupported_record_field_type(context),
                ));
            }
        } else if self.at_text(",")
            || self.at_text("}")
            || self.at_eof()
            || self.at_top_level_decl_start()
        {
            children.push(SyntaxElement::Node(self.parse_type()));
        } else {
            children.push(SyntaxElement::Node(
                self.recover_unsupported_record_field_type(context),
            ));
        }
        self.expect_text(",", &mut children);
        SyntaxNode::new(SyntaxKind::RecordField, children)
    }

    fn parse_enum_decl(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        if self.at_visibility() {
            children.push(SyntaxElement::Node(self.parse_visibility()));
        }
        self.expect_text("enum", &mut children);
        children.push(SyntaxElement::Node(self.parse_identifier()));
        if self.at_text("<") || self.at_text("where") {
            children.push(SyntaxElement::Node(self.recover_top_level()));
            return SyntaxNode::new(SyntaxKind::EnumDecl, children);
        }
        children.push(SyntaxElement::Node(self.parse_enum_body()));
        SyntaxNode::new(SyntaxKind::EnumDecl, children)
    }

    fn parse_enum_body(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        self.expect_text("{", &mut children);
        while !self.at_text("}") && !self.at_eof() && !self.at_top_level_decl_start() {
            if self.at_text("@") {
                children.push(SyntaxElement::Node(self.recover_unsupported_attribute()));
            } else if !self.at_enum_variant_start() {
                children.push(SyntaxElement::Node(
                    self.recover_unsupported_enum_region(EnumRecoveryContext::Body),
                ));
            } else {
                children.push(SyntaxElement::Node(self.parse_enum_variant()));
            }
        }
        self.expect_text("}", &mut children);
        SyntaxNode::new(SyntaxKind::EnumBody, children)
    }

    fn parse_enum_variant(&mut self) -> SyntaxNode {
        let mut children = vec![SyntaxElement::Node(self.parse_identifier())];
        if self.at_text("{") || self.at_record_field_boundary() {
            self.expect_text("{", &mut children);
            while !self.at_text("}")
                && !self.at_eof()
                && !self.at_enum_variant_boundary()
                && !self.at_top_level_decl_start()
                && !self.at_text(",")
            {
                if self.at_text("@") {
                    children.push(SyntaxElement::Node(self.recover_unsupported_attribute()));
                } else if self.at_record_field_start() {
                    children.push(SyntaxElement::Node(
                        self.parse_record_field(RecordFieldContext::EnumPayload),
                    ));
                } else {
                    children.push(SyntaxElement::Node(
                        self.recover_unsupported_enum_region(EnumRecoveryContext::Payload),
                    ));
                }
            }
            self.expect_text("}", &mut children);
        } else if self.at_text("(") {
            children.push(SyntaxElement::Node(
                self.recover_unsupported_tuple_enum_payload(),
            ));
        }
        self.expect_text(",", &mut children);
        SyntaxNode::new(SyntaxKind::EnumVariant, children)
    }

    fn parse_visibility(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        self.bump_expected(&mut children);
        SyntaxNode::new(SyntaxKind::Visibility, children)
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
                "the implemented Phase 1 subset requires at least one typed parameter",
            )));
        } else if !self.at_eof() {
            children.push(SyntaxElement::Node(self.parse_parameter()));
            while self.at_text(",") {
                if self.significant_token_is_followed_by(")") {
                    children.push(SyntaxElement::Node(self.reject_current_token(
                        "trailing parameter commas are outside the implemented Phase 1 subset",
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
        if self.at_text("Self") {
            self.bump_expected(&mut children);
        } else if self.at_identifier() {
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
        let mut has_statement = false;
        while self.at_statement_start() {
            children.push(SyntaxElement::Node(self.parse_statement()));
            has_statement = true;
        }
        if self.at_text("}") {
            if !has_statement {
                children.push(SyntaxElement::Node(self.reject_empty_region(
                    "the implemented Phase 1 subset requires a trailing expression",
                )));
            }
        } else if !self.at_eof() && !self.at_top_level_decl_start() {
            if self.at_expression_start() {
                children.push(SyntaxElement::Node(self.parse_expression()));
            } else {
                children.push(SyntaxElement::Node(self.recover_block_tail()));
            }
        }
        if !self.at_text("}") && !self.at_eof() && !self.at_top_level_decl_start() {
            children.push(SyntaxElement::Node(self.recover_block_tail()));
        }
        self.expect_text("}", &mut children);
        SyntaxNode::new(SyntaxKind::Block, children)
    }

    fn parse_statement(&mut self) -> SyntaxNode {
        let statement = if self.at_text("return") {
            self.parse_return_statement()
        } else {
            self.parse_let_statement()
        };
        SyntaxNode::new(SyntaxKind::Statement, vec![SyntaxElement::Node(statement)])
    }

    fn parse_return_statement(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        self.expect_text("return", &mut children);
        if self.at_expression_start() || self.at_text("-") {
            let expression_cursor = self.cursor;
            let diagnostic_count = self.diagnostics.len();
            let expression = self.parse_expression();
            if !self.at_return_expression_boundary() && !self.at_expression_start() {
                if self.diagnostics.len() > diagnostic_count {
                    let significant = self
                        .current_significant()
                        .expect("unsupported return expression recovery is not called at EOF")
                        .clone();
                    self.cursor = expression_cursor;
                    self.diagnostics.truncate(diagnostic_count);
                    children.push(SyntaxElement::Node(
                        self.recover_unsupported_return_expression_at(significant),
                    ));
                } else {
                    children.push(SyntaxElement::Node(expression));
                    children.push(SyntaxElement::Node(
                        self.recover_unsupported_return_expression(),
                    ));
                }
            } else {
                children.push(SyntaxElement::Node(expression));
            }
        } else if !self.at_return_expression_boundary() {
            children.push(SyntaxElement::Node(
                self.recover_unsupported_return_expression(),
            ));
        }
        self.expect_text(";", &mut children);
        SyntaxNode::new(SyntaxKind::ReturnStatement, children)
    }

    fn recover_unsupported_return_expression(&mut self) -> SyntaxNode {
        let significant = self
            .current_significant()
            .expect("unsupported return expression recovery is not called at EOF")
            .clone();
        self.recover_unsupported_return_expression_at(significant)
    }

    fn recover_unsupported_return_expression_at(&mut self, significant: Token) -> SyntaxNode {
        let mut children = Vec::new();
        if significant.kind != TokenKind::Invalid {
            self.push_diagnostic(
                &UNSUPPORTED_SYNTAX,
                significant.span,
                "return expression is outside the implemented Phase 1 subset".to_owned(),
                None,
                Some(significant.text),
            );
        }

        self.take_trivia(&mut children);
        let mut angle_depth = 0_u32;
        let mut paren_depth = 0_u32;
        let mut bracket_depth = 0_u32;
        let mut brace_depth = 0_u32;
        let mut consumed = false;
        while !self.at_eof() {
            let significant_index = self.significant_index();
            let at_outer_boundary =
                angle_depth == 0 && paren_depth == 0 && bracket_depth == 0 && brace_depth == 0;
            let at_statement_semicolon = self.at_text(";")
                && (at_outer_boundary
                    || (brace_depth == 0
                        && !self.return_delimiters_close_before_block_boundary(
                            significant_index + 1,
                            angle_depth,
                            paren_depth,
                            bracket_depth,
                            brace_depth,
                        )));
            let at_unclosed_delimiter_boundary = !at_outer_boundary
                && (self.at_statement_start()
                    || self.at_top_level_decl_start()
                    || (self.has_line_break_before(significant_index)
                        && self.at_expression_start()))
                && !self.return_delimiters_close_before_block_boundary(
                    significant_index,
                    angle_depth,
                    paren_depth,
                    bracket_depth,
                    brace_depth,
                );
            if consumed
                && (at_statement_semicolon
                    || at_unclosed_delimiter_boundary
                    || (at_outer_boundary
                        && (self.at_statement_start() || self.at_top_level_decl_start()))
                    || (brace_depth == 0 && self.at_text("}")))
            {
                break;
            }

            let token = self.tokens[self.cursor].clone();
            if !token.kind.is_trivia() {
                match token.text.as_str() {
                    "<" => angle_depth += 1,
                    ">" => angle_depth = angle_depth.saturating_sub(1),
                    ">>" => angle_depth = angle_depth.saturating_sub(2),
                    "(" => paren_depth += 1,
                    ")" => paren_depth = paren_depth.saturating_sub(1),
                    "[" => bracket_depth += 1,
                    "]" => bracket_depth = bracket_depth.saturating_sub(1),
                    "{" => brace_depth += 1,
                    "}" => brace_depth = brace_depth.saturating_sub(1),
                    _ => {}
                }
                consumed = true;
            }
            self.bump(&mut children);
        }
        SyntaxNode::new(SyntaxKind::Error, children)
    }

    fn return_delimiters_close_before_block_boundary(
        &self,
        start_index: usize,
        mut angle_depth: u32,
        mut paren_depth: u32,
        mut bracket_depth: u32,
        mut brace_depth: u32,
    ) -> bool {
        let mut index = self.next_significant_index(start_index);
        loop {
            let Some(token) = self.tokens.get(index) else {
                return false;
            };
            if token.kind == TokenKind::Eof {
                return false;
            }

            match token.text.as_str() {
                "<" => angle_depth += 1,
                ">" => angle_depth = angle_depth.saturating_sub(1),
                ">>" => angle_depth = angle_depth.saturating_sub(2),
                "(" => paren_depth += 1,
                ")" => paren_depth = paren_depth.saturating_sub(1),
                "[" => bracket_depth += 1,
                "]" => bracket_depth = bracket_depth.saturating_sub(1),
                "{" => brace_depth += 1,
                "}" if brace_depth == 0 => return false,
                "}" => brace_depth -= 1,
                _ => {}
            }
            if angle_depth == 0 && paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 {
                return true;
            }
            index = self.next_significant_index(index + 1);
        }
    }

    fn has_line_break_before(&self, significant_index: usize) -> bool {
        self.tokens[self.cursor..significant_index]
            .iter()
            .any(|token| token.text.contains(['\n', '\r']))
    }

    fn parse_let_statement(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        self.expect_text("let", &mut children);
        children.push(SyntaxElement::Node(self.parse_pattern()));
        if self.at_text(":") {
            self.bump_expected(&mut children);
            if self.at_type_start() {
                children.push(SyntaxElement::Node(self.parse_type()));
                if self.at_unsupported_let_type_suffix() {
                    children.push(SyntaxElement::Node(self.recover_unsupported_let_type()));
                }
            } else if self.at_let_type_boundary() {
                children.push(SyntaxElement::Node(self.parse_type()));
            } else {
                children.push(SyntaxElement::Node(self.recover_unsupported_let_type()));
            }
        }
        self.expect_text("=", &mut children);
        children.push(SyntaxElement::Node(self.parse_expression()));
        self.expect_text(";", &mut children);
        SyntaxNode::new(SyntaxKind::LetStatement, children)
    }

    fn parse_pattern(&mut self) -> SyntaxNode {
        let binding_is_supported = self.at_identifier()
            && !self.significant_token_is_followed_by(".")
            && !self.significant_token_is_followed_by("{")
            && !self.significant_token_is_followed_by("(");
        let child = if binding_is_supported
            || self.at_text(":")
            || self.at_text("=")
            || self.at_text(";")
            || self.at_text("}")
            || self.at_eof()
        {
            SyntaxNode::new(
                SyntaxKind::BindingPattern,
                vec![SyntaxElement::Node(self.parse_identifier())],
            )
        } else {
            self.recover_unsupported_let_pattern()
        };
        SyntaxNode::new(SyntaxKind::Pattern, vec![SyntaxElement::Node(child)])
    }

    fn recover_unsupported_let_pattern(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        let significant = self
            .current_significant()
            .expect("unsupported let pattern recovery is not called at EOF")
            .clone();
        self.push_diagnostic(
            &UNSUPPORTED_SYNTAX,
            significant.span,
            "patterns other than identifier bindings are outside the implemented Phase 1 subset"
                .to_owned(),
            None,
            Some(significant.text),
        );

        self.take_trivia(&mut children);
        let mut paren_depth = 0_u32;
        let mut bracket_depth = 0_u32;
        let mut brace_depth = 0_u32;
        let mut consumed = false;
        while !self.at_eof() {
            let at_outer_boundary = paren_depth == 0 && bracket_depth == 0 && brace_depth == 0;
            if consumed
                && (self.at_text("=")
                    || self.at_text(";")
                    || self.at_text("let")
                    || self.at_top_level_decl_start()
                    || (brace_depth == 0 && self.at_text("}"))
                    || (at_outer_boundary && self.at_text(":")))
            {
                break;
            }

            let token = self.tokens[self.cursor].clone();
            if !token.kind.is_trivia() {
                match token.text.as_str() {
                    "(" => paren_depth += 1,
                    ")" => paren_depth = paren_depth.saturating_sub(1),
                    "[" => bracket_depth += 1,
                    "]" => bracket_depth = bracket_depth.saturating_sub(1),
                    "{" => brace_depth += 1,
                    "}" => brace_depth = brace_depth.saturating_sub(1),
                    _ => {}
                }
                consumed = true;
            }
            self.bump(&mut children);
        }
        SyntaxNode::new(SyntaxKind::Error, children)
    }

    fn recover_unsupported_let_type(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        let significant = self
            .current_significant()
            .expect("unsupported let type recovery is not called at EOF")
            .clone();
        self.push_diagnostic(
            &UNSUPPORTED_SYNTAX,
            significant.span,
            "let binding type is outside the implemented Phase 1 subset".to_owned(),
            None,
            Some(significant.text),
        );

        self.take_trivia(&mut children);
        let mut paren_depth = 0_u32;
        let mut bracket_depth = 0_u32;
        let mut brace_depth = 0_u32;
        let mut consumed = false;
        while !self.at_eof() {
            let at_outer_boundary = paren_depth == 0 && bracket_depth == 0 && brace_depth == 0;
            if consumed
                && ((at_outer_boundary && (self.at_text("=") || self.at_text(";")))
                    || self.at_text("let")
                    || self.at_top_level_decl_start()
                    || (brace_depth == 0 && self.at_text("}")))
            {
                break;
            }

            let token = self.tokens[self.cursor].clone();
            if !token.kind.is_trivia() {
                match token.text.as_str() {
                    "(" => paren_depth += 1,
                    ")" => paren_depth = paren_depth.saturating_sub(1),
                    "[" => bracket_depth += 1,
                    "]" => bracket_depth = bracket_depth.saturating_sub(1),
                    "{" => brace_depth += 1,
                    "}" => brace_depth = brace_depth.saturating_sub(1),
                    _ => {}
                }
                consumed = true;
            }
            self.bump(&mut children);
        }
        SyntaxNode::new(SyntaxKind::Error, children)
    }

    fn parse_expression(&mut self) -> SyntaxNode {
        SyntaxNode::new(
            SyntaxKind::Expression,
            vec![SyntaxElement::Node(self.parse_additive_expression())],
        )
    }

    fn parse_additive_expression(&mut self) -> SyntaxNode {
        let mut children = vec![SyntaxElement::Node(self.parse_multiplicative_expression())];
        while self.at_text("+") || self.at_text("-") {
            self.bump_expected(&mut children);
            children.push(SyntaxElement::Node(self.parse_multiplicative_expression()));
        }
        SyntaxNode::new(SyntaxKind::AdditiveExpression, children)
    }

    fn parse_multiplicative_expression(&mut self) -> SyntaxNode {
        let mut children = vec![SyntaxElement::Node(self.parse_unary_expression())];
        while self.at_text("*") || self.at_text("/") || self.at_text("%") {
            self.bump_expected(&mut children);
            children.push(SyntaxElement::Node(self.parse_unary_expression()));
        }
        SyntaxNode::new(SyntaxKind::MultiplicativeExpression, children)
    }

    fn parse_unary_expression(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        if self.at_text("-") {
            children.push(SyntaxElement::Node(self.reject_current_token(
                "unary operators are outside the implemented Phase 1 subset",
            )));
        }
        children.push(SyntaxElement::Node(self.parse_postfix_expression()));
        SyntaxNode::new(SyntaxKind::UnaryExpression, children)
    }

    fn parse_postfix_expression(&mut self) -> SyntaxNode {
        let mut children = vec![SyntaxElement::Node(self.parse_primary_expression())];
        while self.at_text("(") {
            children.push(SyntaxElement::Node(SyntaxNode::new(
                SyntaxKind::PostfixSuffix,
                vec![SyntaxElement::Node(self.parse_call_suffix())],
            )));
        }
        SyntaxNode::new(SyntaxKind::PostfixExpression, children)
    }

    fn parse_call_suffix(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        self.expect_text("(", &mut children);
        if !self.at_text(")") {
            children.push(SyntaxElement::Node(self.parse_argument()));
            while self.at_text(",") {
                self.bump_expected(&mut children);
                if self.at_text(")") {
                    break;
                }
                children.push(SyntaxElement::Node(self.parse_argument()));
            }
        }
        self.expect_text(")", &mut children);
        SyntaxNode::new(SyntaxKind::CallSuffix, children)
    }

    fn parse_argument(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        if self.at_identifier() && self.significant_token_is_followed_by(":") {
            children.push(SyntaxElement::Node(self.parse_identifier()));
            self.expect_text(":", &mut children);
        }
        if self.at_expression_start() {
            children.push(SyntaxElement::Node(self.parse_expression()));
        } else {
            self.insert_missing("expression", &mut children);
        }
        SyntaxNode::new(SyntaxKind::Argument, children)
    }

    fn parse_primary_expression(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        if self.at_literal() {
            children.push(SyntaxElement::Node(self.parse_literal()));
        } else if self.at_identifier() {
            children.push(SyntaxElement::Node(SyntaxNode::new(
                SyntaxKind::PathExpression,
                vec![SyntaxElement::Node(SyntaxNode::new(
                    SyntaxKind::QualifiedName,
                    vec![SyntaxElement::Node(self.parse_identifier())],
                ))],
            )));
        } else if self.at_text("(") {
            children.push(SyntaxElement::Node(self.parse_parenthesized_expression()));
        } else {
            self.insert_missing("expression", &mut children);
        }
        SyntaxNode::new(SyntaxKind::PrimaryExpression, children)
    }

    fn parse_literal(&mut self) -> SyntaxNode {
        let mut literal_children = Vec::new();
        let leaf_kind = match self.current_significant() {
            Some(token) if token.kind == TokenKind::Integer => Some(SyntaxKind::IntegerLiteral),
            Some(token) if token.kind == TokenKind::String => Some(SyntaxKind::StringLiteral),
            _ => None,
        };
        if let Some(kind) = leaf_kind {
            let mut leaf_children = Vec::new();
            if kind == SyntaxKind::StringLiteral
                && self
                    .current_significant()
                    .is_some_and(|token| token.text.contains('\\'))
            {
                leaf_children.push(SyntaxElement::Node(self.reject_current_token(
                    "string escapes are outside the implemented Phase 1 subset",
                )));
            } else {
                self.bump_expected(&mut leaf_children);
            }
            literal_children.push(SyntaxElement::Node(SyntaxNode::new(kind, leaf_children)));
        } else {
            self.bump_expected(&mut literal_children);
        }
        SyntaxNode::new(SyntaxKind::Literal, literal_children)
    }

    fn parse_parenthesized_expression(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        self.expect_text("(", &mut children);
        if self.at_expression_start() {
            children.push(SyntaxElement::Node(self.parse_expression()));
        } else {
            self.insert_missing("expression", &mut children);
        }
        self.expect_text(")", &mut children);
        SyntaxNode::new(SyntaxKind::ParenthesizedExpression, children)
    }

    fn recover_top_level(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        let significant = self
            .top_level_error_token()
            .expect("top-level recovery is not called at EOF");
        let stop_unterminated_import_at_declaration = significant.text == "import";
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
            if consumed
                && (brace_depth == 0 || stop_unterminated_import_at_declaration)
                && self.at_top_level_decl_start()
            {
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

    fn recover_unsupported_record_field_type(&mut self, context: RecordFieldContext) -> SyntaxNode {
        let mut children = Vec::new();
        let significant = self
            .current_significant()
            .expect("record field type recovery is not called at EOF")
            .clone();
        self.push_diagnostic(
            &UNSUPPORTED_SYNTAX,
            significant.span,
            "record field type is outside the implemented Phase 1 subset".to_owned(),
            None,
            Some(significant.text),
        );

        self.take_trivia(&mut children);
        let mut angle_depth = 0_u32;
        let mut paren_depth = 0_u32;
        let mut bracket_depth = 0_u32;
        let mut brace_depth = 0_u32;
        let mut previous_significant_text: Option<String> = None;
        while !self.at_eof() {
            let at_outer_boundary =
                angle_depth == 0 && paren_depth == 0 && bracket_depth == 0 && brace_depth == 0;
            let starts_effect_row =
                self.at_text("{") && previous_significant_text.as_deref() == Some("uses");
            if (brace_depth == 0 && self.at_text("}"))
                || (at_outer_boundary
                    && (self.at_text(",")
                        || self.at_record_field_boundary()
                        || (context == RecordFieldContext::EnumPayload
                            && self.at_enum_variant_boundary()
                            && !starts_effect_row)
                        || self.at_top_level_decl_start()))
            {
                break;
            }

            let token = self.tokens[self.cursor].clone();
            if !token.kind.is_trivia() {
                match token.text.as_str() {
                    "<" => angle_depth += 1,
                    ">" => angle_depth = angle_depth.saturating_sub(1),
                    ">>" => angle_depth = angle_depth.saturating_sub(2),
                    "(" => paren_depth += 1,
                    ")" => paren_depth = paren_depth.saturating_sub(1),
                    "[" => bracket_depth += 1,
                    "]" => bracket_depth = bracket_depth.saturating_sub(1),
                    "{" => brace_depth += 1,
                    "}" => brace_depth = brace_depth.saturating_sub(1),
                    _ => {}
                }
                previous_significant_text = Some(token.text.clone());
            }
            self.bump(&mut children);
        }
        SyntaxNode::new(SyntaxKind::Error, children)
    }

    fn recover_unsupported_attribute(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        let significant = self
            .current_significant()
            .expect("attribute recovery starts at an attribute")
            .clone();
        self.push_diagnostic(
            &UNSUPPORTED_SYNTAX,
            significant.span,
            "attributes are outside the implemented Phase 1 subset".to_owned(),
            None,
            Some(significant.text),
        );
        self.bump_expected(&mut children);
        if self.at_identifier() {
            self.bump_expected(&mut children);
        }
        if self.at_text("(") {
            self.recover_balanced_parentheses(&mut children);
        }
        SyntaxNode::new(SyntaxKind::Error, children)
    }

    fn recover_unsupported_tuple_enum_payload(&mut self) -> SyntaxNode {
        let mut children = Vec::new();
        let significant = self
            .current_significant()
            .expect("tuple enum payload recovery starts at an opening parenthesis")
            .clone();
        self.push_diagnostic(
            &UNSUPPORTED_SYNTAX,
            significant.span,
            "tuple-style enum payloads are outside the implemented Phase 1 subset".to_owned(),
            None,
            Some(significant.text),
        );

        self.recover_balanced_parentheses(&mut children);
        SyntaxNode::new(SyntaxKind::Error, children)
    }

    fn recover_balanced_parentheses(&mut self, children: &mut Vec<SyntaxElement>) {
        self.take_trivia(children);
        let mut depth = 0_u32;
        while !self.at_eof() {
            if depth > 0 && (self.at_text("}") || self.at_top_level_decl_start()) {
                break;
            }
            let token = self.tokens[self.cursor].clone();
            if !token.kind.is_trivia() {
                if token.text == "(" {
                    depth += 1;
                } else if token.text == ")" {
                    depth = depth.saturating_sub(1);
                }
            }
            self.bump(children);
            if depth == 0 {
                break;
            }
        }
    }

    fn recover_unsupported_enum_region(&mut self, context: EnumRecoveryContext) -> SyntaxNode {
        let mut children = Vec::new();
        let significant = self
            .current_significant()
            .expect("enum-local recovery is not called at a boundary")
            .clone();
        let region = match context {
            EnumRecoveryContext::Body => "enum body",
            EnumRecoveryContext::Payload => "enum payload",
        };
        self.push_diagnostic(
            &UNSUPPORTED_SYNTAX,
            significant.span,
            format!("{region} contains syntax outside the implemented Phase 1 subset"),
            None,
            Some(significant.text),
        );

        self.take_trivia(&mut children);
        let mut paren_depth = 0_u32;
        let mut bracket_depth = 0_u32;
        let mut brace_depth = 0_u32;
        let mut angle_depth = 0_u32;
        let mut consumed = false;
        while !self.at_eof() {
            if consumed && brace_depth == 0 && (self.at_text("}") || self.at_top_level_decl_start())
            {
                break;
            }
            let at_outer_boundary =
                paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 && angle_depth == 0;
            if consumed && at_outer_boundary {
                if self.at_text(",") {
                    self.bump_expected(&mut children);
                    break;
                }
                let at_context_boundary = match context {
                    EnumRecoveryContext::Body => {
                        self.at_text("}")
                            || self.at_top_level_decl_start()
                            || self.at_enum_variant_boundary()
                            || self.at_text("{")
                    }
                    EnumRecoveryContext::Payload => {
                        self.at_text("}")
                            || self.at_top_level_decl_start()
                            || self.at_enum_variant_boundary()
                            || self.at_record_field_boundary()
                    }
                };
                if at_context_boundary {
                    break;
                }
            }

            let token = self.tokens[self.cursor].clone();
            if !token.kind.is_trivia() {
                match token.text.as_str() {
                    "(" => paren_depth += 1,
                    ")" => paren_depth = paren_depth.saturating_sub(1),
                    "[" => bracket_depth += 1,
                    "]" => bracket_depth = bracket_depth.saturating_sub(1),
                    "{" => brace_depth += 1,
                    "}" => brace_depth = brace_depth.saturating_sub(1),
                    "<" => angle_depth += 1,
                    ">" => angle_depth = angle_depth.saturating_sub(1),
                    ">>" => angle_depth = angle_depth.saturating_sub(2),
                    _ => {}
                }
                consumed = true;
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

    fn at_record_start(&self) -> bool {
        if self.at_text("record") {
            return true;
        }
        if !self.at_visibility() {
            return false;
        }
        let current = self.significant_index();
        let next = self.next_significant_index(current + 1);
        self.tokens
            .get(next)
            .is_some_and(|token| token.text == "record")
    }

    fn at_enum_start(&self) -> bool {
        if self.at_text("enum") {
            return true;
        }
        if !self.at_visibility() {
            return false;
        }
        let current = self.significant_index();
        let next = self.next_significant_index(current + 1);
        self.tokens
            .get(next)
            .is_some_and(|token| token.text == "enum")
    }

    fn at_top_level_decl_start(&self) -> bool {
        self.at_enum_start() || self.at_record_start() || self.at_function_start()
    }

    fn at_enum_variant_start(&self) -> bool {
        self.at_identifier() || self.at_text("{") || self.at_text(",")
    }

    fn at_enum_variant_boundary(&self) -> bool {
        if self.at_text("{") {
            return true;
        }
        if !self.at_identifier() {
            return false;
        }
        let current = self.significant_index();
        let next = self.next_significant_index(current + 1);
        self.tokens
            .get(next)
            .is_some_and(|token| matches!(token.text.as_str(), "," | "{" | "("))
    }

    fn at_record_field_start(&self) -> bool {
        if self.at_identifier() || self.at_text(":") {
            return true;
        }
        if !self.at_visibility() {
            return false;
        }
        let current = self.significant_index();
        let next = self.next_significant_index(current + 1);
        self.tokens
            .get(next)
            .is_some_and(|token| token.kind == TokenKind::Identifier || token.text == ":")
    }

    fn at_record_field_boundary(&self) -> bool {
        let current = self.significant_index();
        let Some(token) = self.tokens.get(current) else {
            return false;
        };
        if token.text == ":" {
            return true;
        }

        let name = if matches!(token.text.as_str(), "pub" | "internal") {
            self.next_significant_index(current + 1)
        } else {
            current
        };
        let Some(name_token) = self.tokens.get(name) else {
            return false;
        };
        if name_token.text == ":" {
            return true;
        }
        if name_token.kind != TokenKind::Identifier {
            return false;
        }

        let colon = self.next_significant_index(name + 1);
        self.tokens
            .get(colon)
            .is_some_and(|token| token.text == ":")
    }

    fn at_visibility(&self) -> bool {
        self.at_text("pub") || self.at_text("internal")
    }

    fn at_type_start(&self) -> bool {
        self.at_identifier() || self.at_text("Self")
    }

    fn at_statement_start(&self) -> bool {
        self.at_text("let") || self.at_text("return")
    }

    fn at_return_expression_boundary(&self) -> bool {
        self.at_text(";")
            || self.at_text("}")
            || self.at_eof()
            || self.at_statement_start()
            || self.at_top_level_decl_start()
    }

    fn at_let_type_boundary(&self) -> bool {
        self.at_text("=")
            || self.at_text(";")
            || self.at_text("}")
            || self.at_text("let")
            || self.at_eof()
            || self.at_top_level_decl_start()
    }

    fn at_unsupported_let_type_suffix(&self) -> bool {
        !self.at_let_type_boundary()
            && (!self.at_expression_start() || self.let_initializer_separator_follows())
    }

    fn let_initializer_separator_follows(&self) -> bool {
        let mut index = self.significant_index();
        let mut paren_depth = 0_u32;
        let mut bracket_depth = 0_u32;
        let mut brace_depth = 0_u32;
        loop {
            let Some(token) = self.tokens.get(index) else {
                return false;
            };
            if token.kind == TokenKind::Eof {
                return false;
            }

            let at_outer_boundary = paren_depth == 0 && bracket_depth == 0 && brace_depth == 0;
            if at_outer_boundary && token.text == "=" {
                return true;
            }
            if (at_outer_boundary && token.text == ";")
                || matches!(token.text.as_str(), "let" | "pub" | "record" | "enum")
                || (brace_depth == 0 && token.text == "}")
            {
                return false;
            }

            match token.text.as_str() {
                "(" => paren_depth += 1,
                ")" => paren_depth = paren_depth.saturating_sub(1),
                "[" => bracket_depth += 1,
                "]" => bracket_depth = bracket_depth.saturating_sub(1),
                "{" => brace_depth += 1,
                "}" => brace_depth = brace_depth.saturating_sub(1),
                _ => {}
            }
            index = self.next_significant_index(index + 1);
        }
    }

    fn at_unsupported_record_field_type_suffix(&self, context: RecordFieldContext) -> bool {
        !(self.at_text(",")
            || self.at_text("}")
            || self.at_eof()
            || self.at_top_level_decl_start()
            || self.at_record_field_start()
            || (context == RecordFieldContext::EnumPayload && self.at_enum_variant_boundary()))
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
        self.at_literal() || self.at_identifier() || self.at_text("(")
    }

    fn at_literal(&self) -> bool {
        self.current_significant().is_some_and(|token| {
            matches!(token.kind, TokenKind::Integer | TokenKind::String)
                || matches!(token.text.as_str(), "true" | "false" | "Unit" | "None")
        })
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
