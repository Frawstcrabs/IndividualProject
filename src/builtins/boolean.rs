use crate::lang_core::interp::{LangResult, LangError, VarValues, Context, Gc, f64_to_string};
use std::cell::RefCell;

pub fn eq_func(_ctx: &mut Context, args: Vec<Gc<VarValues>>) -> LangResult<Gc<VarValues>> {
    if args.len() < 2 {
        return Err(LangError::Throw(
            Gc::new(RefCell::new(
                VarValues::Str(format!("<eq:expected 2 args, got {}>", args.len()))
            ))
        ));
    }

    let mut item1 = &args[0];
    for item2 in &args[1..] {
        let comp = match (&*item1.borrow(), &*item2.borrow()) {
            (VarValues::Nil, VarValues::Nil) => {
                true
            },
            (VarValues::Nil, VarValues::Str(s)) |
            (VarValues::Str(s), VarValues::Nil) => {
                s.is_empty()
            },
            (VarValues::Str(s1), VarValues::Str(s2)) => {
                s1 == s2
            },
            (VarValues::Num(n1), VarValues::Num(n2)) => {
                n1 == n2
            },
            (VarValues::Num(n), VarValues::Str(s)) |
            (VarValues::Str(s), VarValues::Num(n)) => {
                s == &f64_to_string(*n)
            },
            (_, _) => {
                false
            },
        };
        if !comp {
            return Ok(Gc::new(RefCell::new(VarValues::Num(0.0))));
        }
        item1 = item2;
    }

    Ok(Gc::new(RefCell::new(VarValues::Num(1.0))))
}