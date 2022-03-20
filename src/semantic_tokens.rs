use lang_frontend::tokenizer::{Span, Token};
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

pub fn make_tokens_semantic(tokens: &[(Token, Span)], rope: &Rope) -> Vec<SemanticToken> {
    // Estos valores nos son utiles a la hora de generar los deltas
    let mut pre_line = 0;
    let mut pre_start = 0;

    tokens
        .iter()
        .filter_map(|(token, span)| {
            // Calculamos el tipo de token
            let token_type = match token {
                Token::Number(_) => SemanticTokenType::NUMBER,
                Token::Text(_) => SemanticTokenType::STRING,
                Token::Op(_) => SemanticTokenType::OPERATOR,
                Token::Ctrl(_) => return None,
                Token::Ident(_) => SemanticTokenType::VARIABLE,
                Token::Bool(_) | Token::While | Token::If | Token::Else => {
                    SemanticTokenType::KEYWORD
                }
            };

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
            Some(SemanticToken {
                delta_line,
                delta_start,
                length: span.len() as u32,
                token_type: LEGEND_TYPE
                    .iter()
                    .position(|item| item == &token_type)
                    .unwrap() as u32,
                token_modifiers_bitset: 0,
            })
        })
        .collect()
}
