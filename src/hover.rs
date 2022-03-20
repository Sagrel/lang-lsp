use lang_frontend::{
    ast::{Ast, Spanned},
    types::Type,
};

pub fn find_match(node: &Spanned<Ast>, pos: usize) -> Option<Type> {
    if node.1.contains(&pos) {
        match &node.0 {
            Ast::Error | Ast::Literal(_) | Ast::Variable(_) => node.2.clone(),
            Ast::Declaration(name, variant) => {
                let wants_all = node.1.start + name.len() > pos;

                match variant.as_ref() {
                    lang_frontend::ast::Declaration::Complete(ty, val) => {
                        if wants_all {
                            return val.2.clone();
                        } else if let Some(t) = find_match(ty, pos) {
                            return Some(t);
                        } else if let Some(t) = find_match(val, pos) {
                            return Some(t);
                        }
                    }
                    lang_frontend::ast::Declaration::OnlyType(ty) => {
                        if let Some(t) = find_match(ty, pos) {
                            return Some(t);
                        }
                    }
                    lang_frontend::ast::Declaration::OnlyValue(val, _) => {
                        if wants_all {
                            return val.2.clone();
                        } else if let Some(t) = find_match(val, pos) {
                            return Some(t);
                        }
                    }
                }
                node.2.clone()
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
                node.2.clone()
            }
            Ast::Binary(l, _, r) => {
                if let Some(t) = find_match(l, pos) {
                    return Some(t);
                }
                if let Some(t) = find_match(r, pos) {
                    return Some(t);
                }
                node.2.clone()
            }
            Ast::While(cond, body) => {
                if let Some(t) = find_match(cond, pos) {
                    return Some(t);
                }
                if let Some(t) = find_match(body, pos) {
                    return Some(t);
                }
                node.2.clone()
            }
            Ast::If(cond, if_body, else_body) => {
                if let Some(t) = find_match(cond, pos) {
                    return Some(t);
                }
                if let Some(t) = find_match(if_body, pos) {
                    return Some(t);
                }
                if let Some(t) = find_match(else_body, pos) {
                    return Some(t);
                }
                node.2.clone()
            }
            Ast::Tuple(args) => {
                for arg in args {
                    if let Some(t) = find_match(arg, pos) {
                        return Some(t);
                    }
                }
                node.2.clone()
            }
            Ast::Block(args) => {
                for arg in args {
                    if let Some(t) = find_match(arg, pos) {
                        return Some(t);
                    }
                }
                node.2.clone()
            }
            Ast::Lambda(args, ret) => {
                for arg in args {
                    if let Some(t) = find_match(arg, pos) {
                        return Some(t);
                    }
                }
                if let Some(t) = find_match(ret, pos) {
                    return Some(t);
                }
                node.2.clone()
            }
        }
    } else {
        None
    }
}
