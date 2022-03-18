use lang_frontend::{ast::*, tokenizer::Span, types::Type};
use std::collections::HashMap;

pub fn get_inlay_hints(node: &Spanned<Ast>, hints: &mut HashMap<Span, Type>) {
    match &node.0 {
        Ast::Declaration(_, variant) => {
            if let Declaration::OnlyValue(value, span) = variant.as_ref() {
                let (_, _, t) = value;
                hints.insert(span.clone(), t.clone().unwrap());
                get_inlay_hints(value, hints);
            }
            // TODO handle other cases
        }
        Ast::Call(caller, args) => {
            get_inlay_hints(caller, hints);
            for node in args {
                get_inlay_hints(node, hints);
            }
        }
        Ast::Binary(l, _, r) => {
            get_inlay_hints(l, hints);
            get_inlay_hints(r, hints);
        }
        Ast::While(cond, body) => {
            get_inlay_hints(cond, hints);
            get_inlay_hints(body, hints);
        }
        Ast::If(cond, if_body, else_body) => {
            get_inlay_hints(cond, hints);
            get_inlay_hints(if_body, hints);
            get_inlay_hints(else_body, hints);
        }
        Ast::Tuple(args) => {
            for node in args {
                get_inlay_hints(node, hints);
            }
        }
        Ast::Block(expresions) => {
            for node in expresions {
                get_inlay_hints(node, hints);
            }
        }
        Ast::Lambda(args, ret) => {
            for node in args {
                get_inlay_hints(node, hints);
            }
            get_inlay_hints(ret, hints);
        }
        _ => (),
    }
}
