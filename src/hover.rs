use lang_frontend::{
    ast::{Anotated, Ast},
    types::Type,
};

pub fn find_match((node, node_span, node_ty): &Anotated<Ast>, pos: usize) -> Option<Type> {
    if node_span.contains(&pos) {
        match &node {
            Ast::Error | Ast::Literal(_) | Ast::Variable(_) => node_ty.clone(),
            Ast::Declaration((_, name_span), _, ty, _, value) => {
                if name_span.contains(&pos) {
                    return node_ty.clone();
                }

                if let Some(ty) = ty {
                    if let Some(t) = find_match(ty, pos) {
                        return Some(t);
                    }
                }

                if let Some(value) = value {
                    if let Some(t) = find_match(value, pos) {
                        return Some(t);
                    }
                }

                None
            }
            Ast::Call(caller, args) => {
                if let Some(t) = find_match(caller, pos) {
                    return Some(t);
                }
                for arg in args {
                    if let Some(t) = find_match(arg, pos) {
                        return Some(t);
                    }
                }
                node_ty.clone()
            }
            Ast::Binary(l, _, r) => {
                if let Some(t) = find_match(l, pos) {
                    return Some(t);
                }
                if let Some(t) = find_match(r, pos) {
                    return Some(t);
                }
                node_ty.clone()
            }
            Ast::While(_, cond, body) => {
                if let Some(t) = find_match(cond, pos) {
                    return Some(t);
                }
                if let Some(t) = find_match(body, pos) {
                    return Some(t);
                }
                node_ty.clone()
            }
            Ast::If(_, cond, if_body, _, else_body) => {
                if let Some(t) = find_match(cond, pos) {
                    return Some(t);
                }
                if let Some(t) = find_match(if_body, pos) {
                    return Some(t);
                }
                if let Some(t) = find_match(else_body, pos) {
                    return Some(t);
                }
                node_ty.clone()
            }
            Ast::Tuple(args) => {
                for arg in args {
                    if let Some(t) = find_match(arg, pos) {
                        return Some(t);
                    }
                }
                node_ty.clone()
            }
            Ast::Block(args) => {
                for arg in args {
                    if let Some(t) = find_match(arg, pos) {
                        return Some(t);
                    }
                }
                node_ty.clone()
            }
            Ast::Lambda(args, _, ret) => {
                for arg in args {
                    if let Some(t) = find_match(arg, pos) {
                        return Some(t);
                    }
                }
                if let Some(t) = find_match(ret, pos) {
                    return Some(t);
                }
                node_ty.clone()
            }
            Ast::Coment(_) => None, // TODO this still makes the pop up say Type: ()
        }
    } else {
        None
    }
}
