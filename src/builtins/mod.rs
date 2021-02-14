use std::collections::HashMap;
use std::cell::RefCell;
use crate::lang_core::interp::{VarValues, Gc, VarRefType, SendSyncRefCell};
use crate::new_value;

mod boolean;
mod math;

macro_rules! add_func {
    ($vars:expr, $func:expr, $($names:expr),+) => {
        {
            let val = VarRefType::Value(new_value!(VarValues::RustFunc($func)));
            add_func!(__impl $vars, val, $($names),+);
        }
    };
    (__impl $vars:expr, $func:expr, $name:expr, $($names:expr),+) => {
        $vars.insert($name.to_string(), Gc::clone(&$func));
        add_func!(__impl $vars, $func, $($names),+);
    };
    (__impl $vars:expr, $func:expr, $name:expr) => {
        $vars.insert($name.to_string(), $func);
    };
}

pub fn register_builtins(vars: &mut HashMap<String, VarRefType>) {
    add_func!(vars, boolean::not_func, "not");
    add_func!(vars, boolean::eq_func, "eq");
    add_func!(vars, boolean::ne_func, "ne");
    add_func!(vars, boolean::lt_func, "lt");
    add_func!(vars, boolean::gt_func, "gt");
    add_func!(vars, boolean::le_func, "le");
    add_func!(vars, boolean::ge_func, "ge");
    add_func!(vars, math::add_func, "add");
    add_func!(vars, math::sub_func, "sub");
    add_func!(vars, math::mul_func, "mul");
    add_func!(vars, math::fdiv_func, "fdiv");
    add_func!(vars, math::mod_func, "mod");
}