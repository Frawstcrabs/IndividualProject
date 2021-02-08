use crate::throw_string;
use crate::lang_core::interp::{
    LangResult,
    LangError,
    VarValues,
    Context,
    Gc
};
use std::cell::RefCell;

pub fn add_func(_ctx: &mut Context, args: Vec<Gc<VarValues>>) -> LangResult<Gc<VarValues>> {
    if args.len() < 2 {
        return throw_string!("<add:expected 2 args, got {}>", args.len());
    }

    let mut ret = 0.0;

    for arg in args {
        match &*arg.borrow() {
            VarValues::Num(n) |
            VarValues::AstStr(_, Some(n))=> {
                ret += *n;
            },
            VarValues::Str(s) => {
                ret += match s.parse::<f64>() {
                    Ok(v) => v,
                    Err(_) => {
                        return throw_string!("<add:invalid num>");
                    }
                };
            },
            _ => {
                return throw_string!("<add:invalid num>");
            },
        }
    }

    Ok(Gc::new(RefCell::new(VarValues::Num(ret))))
}