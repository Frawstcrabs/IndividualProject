use crate::throw_string;
use crate::lang_core::interp::{
    LangResult,
    LangError,
    VarValues,
    Context,
    Gc,
    f64_to_string,
};
use std::cell::RefCell;

pub fn eq_func(_ctx: &mut Context, args: Vec<Gc<VarValues>>) -> LangResult<Gc<VarValues>> {
    if args.len() < 2 {
        return throw_string!("<eq:expected 2 args, got {}>", args.len());
    }

    let mut item1 = &args[0];
    for item2 in &args[1..] {
        use VarValues::*;
        let comp = match (&*item1.borrow(), &*item2.borrow()) {
            (Nil, Nil) => {
                true
            },
            (Nil, Str(s)) |
            (Str(s), Nil) |
            (Nil, AstStr(s, _)) |
            (AstStr(s, _), Nil) => {
                s.is_empty()
            },
            (Str(s1), Str(s2)) |
            (AstStr(s1, _), Str(s2)) |
            (Str(s1), AstStr(s2, _)) |
            (AstStr(s1, None), AstStr(s2, None)) => {
                s1 == s2
            },
            (Num(n1), Num(n2)) |
            (AstStr(_, Some(n1)), Num(n2)) |
            (Num(n1), AstStr(_, Some(n2))) |
            (AstStr(_, Some(n1)), AstStr(_, Some(n2))) => {
                n1 == n2
            },
            (Str(s), Num(n)) |
            (Num(n), Str(s)) => {
                s == &f64_to_string(*n)
            },
            (_, _) => {
                false
            },
        };
        if !comp {
            return Ok(Gc::new(RefCell::new(Num(0.0))));
        }
        item1 = item2;
    }

    Ok(Gc::new(RefCell::new(VarValues::Num(1.0))))
}