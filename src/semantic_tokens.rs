use lang_frontend::{ast::*, inferer::Inferer, token::*, types::Type};
use ropey::Rope;
use tower_lsp::lsp_types::{SemanticToken, SemanticTokenType};

pub const LEGEND_TYPE: &[SemanticTokenType] = &[
    SemanticTokenType::FUNCTION,
    SemanticTokenType::TYPE,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::STRING,
    SemanticTokenType::COMMENT,
    SemanticTokenType::NUMBER,
    SemanticTokenType::KEYWORD,
    SemanticTokenType::OPERATOR,
    SemanticTokenType::PARAMETER,
    SemanticTokenType::ENUM_MEMBER,
];

pub fn make_tokens_semantic(
    tokens: &[Spanned<SemanticTokenType>],
    rope: &Rope,
) -> Vec<SemanticToken> {
    // Estos valores nos son utiles a la hora de generar los deltas
    let mut pre_line = 0;
    let mut pre_start = 0;

    tokens
        .iter()
        .map(|(token_type, span)| {
            // Calculamos los deltas del token
            let line = rope.try_byte_to_line(span.start as usize).unwrap() as u32;
            let first = rope.try_line_to_char(line as usize).unwrap() as u32;
            let start = rope.try_byte_to_char(span.start as usize).unwrap() as u32 - first;
            let delta_line = line - pre_line;
            let delta_start = if delta_line == 0 {
                start - pre_start
            } else {
                start
            };
            pre_line = line;
            pre_start = start;

            // Creamos el SemanticToken con toda la información
            SemanticToken {
                delta_line,
                delta_start,
                length: span.len() as u32,
                token_type: LEGEND_TYPE
                    .iter()
                    .position(|item| item == token_type)
                    .unwrap() as u32,
                token_modifiers_bitset: 0,
            }
        })
        .collect()
}

fn make_tokens_of_pattern(
    pattern: &Anotated<Pattern>,
    type_table: &[Type],
    tokens: &mut Vec<Spanned<SemanticTokenType>>,
) {
    match &pattern.0 {
        Pattern::Var((_, span)) => {
            if let Type::Fn(_, _) =
                Inferer::get_most_concrete_type(pattern.2.as_ref().unwrap(), type_table)
            {
                tokens.push((SemanticTokenType::FUNCTION, span.clone()))
            } else {
                tokens.push((SemanticTokenType::VARIABLE, span.clone()))
            }
        }
        Pattern::Tuple(args) => {
            for arg in args {
                make_tokens_of_pattern(arg, type_table, tokens)
            }
        }
    }
}

// TODO keep info of scoping so we can know when a variable is a parameter and mark it as such
pub fn make_tokens_of_ast(
    node: &Anotated<Ast>,
    type_table: &[Type],
    tokens: &mut Vec<Spanned<SemanticTokenType>>,
) {
    use Token::*;

    match &node.0 {
        Ast::Error => (),
        Ast::Literal((Bool(_), span)) => {
            tokens.push((SemanticTokenType::ENUM_MEMBER, span.clone()))
        }
        Ast::Literal((Number(_), span)) => tokens.push((SemanticTokenType::NUMBER, span.clone())),
        Ast::Literal((Text(_), span)) => tokens.push((SemanticTokenType::STRING, span.clone())),
        Ast::Variable((Ident(_), span)) => {
            if let Type::Fn(_, _) =
                Inferer::get_most_concrete_type(node.2.as_ref().unwrap(), type_table)
            {
                tokens.push((SemanticTokenType::FUNCTION, span.clone()))
            } else {
                tokens.push((SemanticTokenType::VARIABLE, span.clone()))
            }
        }
        Ast::Declaration(pattern, _, ty, _, value) => {
            make_tokens_of_pattern(pattern, type_table, tokens);

            if let Some(ty) = ty {
                tokens.push((SemanticTokenType::TYPE, ty.1.clone()));
            }

            if let Some(value) = value {
                make_tokens_of_ast(value, type_table, tokens);
            }
        }
        Ast::Call(caller, args) => {
            make_tokens_of_ast(caller, type_table, tokens);
            for arg in args {
                make_tokens_of_ast(arg, type_table, tokens);
            }
        }
        Ast::Binary(l, (Token::Op(name), span), r) => {
            make_tokens_of_ast(l, type_table, tokens);
            let tk_ty = match name.as_str() {
                "and" | "or" | "not" => SemanticTokenType::KEYWORD,
                _ => SemanticTokenType::OPERATOR,
            };
            tokens.push((tk_ty, span.clone()));
            make_tokens_of_ast(r, type_table, tokens);
        }
        Ast::While((_, span), cond, body) => {
            tokens.push((SemanticTokenType::KEYWORD, span.clone()));
            make_tokens_of_ast(cond, type_table, tokens);
            make_tokens_of_ast(body, type_table, tokens);
        }
        Ast::If((_, span), cond, if_body, else_tk, else_body) => {
            tokens.push((SemanticTokenType::KEYWORD, span.clone()));
            make_tokens_of_ast(cond, type_table, tokens);
            make_tokens_of_ast(if_body, type_table, tokens);
            if let Some((_, span)) = else_tk {
                tokens.push((SemanticTokenType::KEYWORD, span.clone()));
            }
            make_tokens_of_ast(else_body, type_table, tokens);
        }
        Ast::Tuple(args) => {
            for arg in args {
                make_tokens_of_ast(arg, type_table, tokens);
            }
        }
        Ast::Block(args) => {
            for arg in args {
                make_tokens_of_ast(arg, type_table, tokens);
            }
        }
        Ast::Lambda(args, (_, span), body) => {
            tokens.push((SemanticTokenType::OPERATOR, span.clone()));
            for arg in args {
                make_tokens_of_ast(arg, type_table, tokens);
            }
            make_tokens_of_ast(body, type_table, tokens);
        }
        Ast::Coment((_, span)) => {
            tokens.push((SemanticTokenType::COMMENT, span.clone()));
        }
        _ => panic!("Yo WTF?"),
    }
}
