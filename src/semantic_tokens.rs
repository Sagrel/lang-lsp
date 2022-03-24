use lang_frontend::{
    ast::{self, *},
    inferer::Inferer,
    token::*,
    types::Type,
};
use ropey::Rope;
use tower_lsp::lsp_types::{SemanticToken, SemanticTokenType};

pub const LEGEND_TYPE: &[SemanticTokenType] = &[
    SemanticTokenType::FUNCTION,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::STRING,
    SemanticTokenType::COMMENT,
    SemanticTokenType::NUMBER,
    SemanticTokenType::KEYWORD,
    SemanticTokenType::OPERATOR,
    SemanticTokenType::PARAMETER,
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

// TODO keep that of scoping so we can know when a variable is a parameter
pub fn make_tokens_of_ast(
    node: &Anotated<Ast>,
    type_table: &[Type],
    tokens: &mut Vec<Spanned<SemanticTokenType>>,
) {
    use Ast::*;
    use Token::*;

    match &node.0 {
        Error => (),
        Literal((Bool(_), span)) => tokens.push((SemanticTokenType::KEYWORD, span.clone())),
        Literal((Number(_), span)) => tokens.push((SemanticTokenType::NUMBER, span.clone())),
        Literal((Text(_), span)) => tokens.push((SemanticTokenType::STRING, span.clone())),
        Variable((Ident(_), span)) => {
            if let Type::Fn(_, _) =
                Inferer::get_most_concrete_type(node.2.as_ref().unwrap(), type_table)
            {
                tokens.push((SemanticTokenType::FUNCTION, span.clone()))
            } else {
                tokens.push((SemanticTokenType::VARIABLE, span.clone()))
            }
        }
        Declaration((_, span), variant) => {
            // TODO Handle all variants
            let t = match variant.as_ref() {
                ast::Declaration::Complete(_, _) => None,
                ast::Declaration::OnlyType(_) => None,
                ast::Declaration::OnlyValue(v, _) => {
                    make_tokens_of_ast(v, type_table, tokens);

                    v.2.clone()
                }
            };
            if let Type::Fn(_, _) = Inferer::get_most_concrete_type(&t.unwrap(), type_table) {
                tokens.push((SemanticTokenType::FUNCTION, span.clone()))
            } else {
                tokens.push((SemanticTokenType::VARIABLE, span.clone()))
            }
        }
        Call(caller, args) => {
            make_tokens_of_ast(caller, type_table, tokens);
            for arg in args {
                make_tokens_of_ast(arg, type_table, tokens);
            }
        }
        Binary(l, (_, span), r) => {
            make_tokens_of_ast(l, type_table, tokens);
            tokens.push((SemanticTokenType::OPERATOR, span.clone()));
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
        Tuple(args) => {
            for arg in args {
                make_tokens_of_ast(arg, type_table, tokens);
            }
        }
        Block(args) => {
            for arg in args {
                make_tokens_of_ast(arg, type_table, tokens);
            }
        }
        Lambda(args, (_, span), body) => {
            tokens.push((SemanticTokenType::OPERATOR, span.clone()));
            for arg in args {
                make_tokens_of_ast(arg, type_table, tokens);
            }
            make_tokens_of_ast(body, type_table, tokens);
        }
        _ => panic!("Yo WTF?"),
    }
}
