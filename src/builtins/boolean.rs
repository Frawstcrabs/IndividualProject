use crate::throw_string;
use crate::lang_core::interp::{
    LangResult,
    LangError,
    VarValues,
    Context,
    Gc,
    f64_to_string,
    new_value,
    borrow_val
};
use crate::builtins::math::val_to_f64;

pub fn test_equality(item1: &Gc<VarValues>, item2: &Gc<VarValues>) -> LangResult<bool> {
    use VarValues::*;
    match (&*borrow_val(item1)?, &*borrow_val(item2)?) {
        (Nil, Nil) => {
            Ok(true)
        },
        (Nil, Str(s)) |
        (Str(s), Nil) |
        (Nil, AstStr(s, _)) |
        (AstStr(s, _), Nil) => {
            Ok(s.is_empty())
        },
        (Num(n1), Num(n2)) |
        (AstStr(_, Some(n1)), Num(n2)) |
        (Num(n1), AstStr(_, Some(n2))) |
        (AstStr(_, Some(n1)), AstStr(_, Some(n2))) => {
            Ok(n1 == n2)
        },
        (Str(s1), Str(s2)) |
        (AstStr(s1, _), Str(s2)) |
        (Str(s1), AstStr(s2, _)) |
        (AstStr(s1, None), AstStr(s2, None)) => {
            Ok(s1 == s2)
        },
        (Str(s), Num(n)) |
        (Num(n), Str(s)) => {
            Ok(s == &f64_to_string(*n))
        },
        (_, _) => {
            Ok(false)
        },
    }
}

pub fn not_func(_ctx: &mut Context, args: Vec<Gc<VarValues>>) -> LangResult<Gc<VarValues>> {
    if args.len() != 1 {
        return throw_string!("<eq:expected 1 arg, got {}>", args.len());
    }
    let bool_val: bool = (&*borrow_val(&args[0])?).into();
    return Ok(
        new_value(
            VarValues::Num(if !bool_val {1.0} else {0.0})
        )
    );
}

pub fn eq_func(_ctx: &mut Context, args: Vec<Gc<VarValues>>) -> LangResult<Gc<VarValues>> {
    if args.len() < 2 {
        return throw_string!("<eq:expected 2 args, got {}>", args.len());
    }

    let mut item1 = &args[0];
    for item2 in &args[1..] {
        use VarValues::*;
        if !test_equality(item1, item2)? {
            return Ok(new_value(Num(0.0)));
        }
        item1 = item2;
    }

    Ok(new_value(VarValues::Num(1.0)))
}

pub fn ne_func(_ctx: &mut Context, args: Vec<Gc<VarValues>>) -> LangResult<Gc<VarValues>> {
    if args.len() < 2 {
        return throw_string!("<ne:expected 2 args, got {}>", args.len());
    }

    let mut item1 = &args[0];
    for item2 in &args[1..] {
        use VarValues::*;
        if test_equality(item1, item2)? {
            return Ok(new_value(Num(0.0)));
        }
        item1 = item2;
    }

    Ok(new_value(VarValues::Num(1.0)))
}

macro_rules! num_comp_func {
    ($func_name:ident, $lang_name:expr, $op:tt) => {
        pub fn $func_name(_ctx: &mut Context, args: Vec<Gc<VarValues>>) -> LangResult<Gc<VarValues>> {
            if args.len() != 2 {
                return throw_string!(concat!("<", $lang_name, ":expected 2 args, got {}>"), args.len());
            }
            use VarValues::Num;
            let mut item1 = val_to_f64(&args[0], $lang_name)?;
            for item2 in &args[1..] {
                let item2 = val_to_f64(item2, $lang_name)?;
                if !(item1 $op item2) {
                    return Ok(new_value(Num(0.0)));
                }
                item1 = item2;
            }

            Ok(new_value(Num(1.0)))
        }
    }
}

num_comp_func!(lt_func, "lt", <);
num_comp_func!(gt_func, "gt", >);
num_comp_func!(le_func, "le", <=);
num_comp_func!(ge_func, "ge", >=);

pub fn and_func(_ctx: &mut Context, args: Vec<Gc<VarValues>>) -> LangResult<Gc<VarValues>> {
    if args.len() < 2 {
        return throw_string!("<and:expected 2+ args, got {}>", args.len());
    }
    for arg in &args[..args.len()-1] {
        let test: bool = (&*borrow_val(arg)?).into();
        if !test {
            return Ok(*arg);
        }
    }
    Ok(args[args.len()-1])
}

pub fn or_func(_ctx: &mut Context, args: Vec<Gc<VarValues>>) -> LangResult<Gc<VarValues>> {
    if args.len() < 2 {
        return throw_string!("<or:expected 2+ args, got {}>", args.len());
    }
    for arg in &args[..args.len()-1] {
        let test: bool = (&*borrow_val(arg)?).into();
        if test {
            return Ok(*arg);
        }
    }
    Ok(args[args.len()-1])
}