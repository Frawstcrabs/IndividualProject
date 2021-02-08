use crate::throw_string;
use crate::lang_core::interp::{
    LangResult,
    LangError,
    VarValues,
    Context,
    Gc,
    string_to_f64,
};
use std::cell::RefCell;

pub(crate) fn val_to_f64(val: &Gc<VarValues>, func_name: &str) -> LangResult<f64> {
    match &*val.borrow() {
        VarValues::Num(n) |
        VarValues::AstStr(_, Some(n))=> {
            Ok(*n)
        },
        VarValues::Str(s) => {
            match string_to_f64(s) {
                Some(v) => Ok(v),
                None => {
                    return throw_string!("<{}:invalid num>", func_name);
                },
            }
        },
        _ => {
            return throw_string!("<{}:invalid num>", func_name);
        },
    }
}

macro_rules! math_func {
    ($func_name:ident, $lang_name:expr, $args_name:ident, $test:expr, $arg_count:expr, $op:tt) => {
        pub fn $func_name(_ctx: &mut Context, $args_name: Vec<Gc<VarValues>>) -> LangResult<Gc<VarValues>> {
            if $test {
                return throw_string!(concat!("<", $lang_name, ":expected ", $arg_count, " args, got {}>"), $args_name.len());
            }

            let mut ret = val_to_f64(&$args_name[0], $lang_name)?;

            for arg in &$args_name[1..] {
                ret = ret $op val_to_f64(arg, $lang_name)?;
            }

            Ok(Gc::new(RefCell::new(VarValues::Num(ret))))
        }
    }
}

math_func!(add_func, "add", args, args.len() < 2, "2+", +);
math_func!(sub_func, "sub", args, args.len() != 2, "2", -);
math_func!(mul_func, "mul", args, args.len() < 2, "2+", *);
math_func!(fdiv_func, "fdiv", args, args.len() != 2, "2", /);
math_func!(mod_func, "mod", args, args.len() != 2, "2", %);