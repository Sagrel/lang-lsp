use lang_frontend::{
    ast::*,
    token::{Span, Token},
    types::Type,
};
use std::collections::HashMap;

pub fn get_inlay_hints(node: &Anotated<Ast>, hints: &mut HashMap<Span, Type>) {
    match &node.0 {
        Ast::Declaration(_, (def_tk, span), _, _, Some(value)) => {
            if &Token::Op(":=".to_string()) == def_tk {
                hints.insert(span.clone(), value.2.clone().unwrap());
            }
            get_inlay_hints(value, hints);
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
        Ast::While(_, cond, body) => {
            get_inlay_hints(cond, hints);
            get_inlay_hints(body, hints);
        }
        Ast::If(_, cond, if_body, _, else_body) => {
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
        Ast::Lambda(args, _, ret) => {
            for node in args {
                get_inlay_hints(node, hints);
            }
            get_inlay_hints(ret, hints);
        }
        _ => (),
    }
}
