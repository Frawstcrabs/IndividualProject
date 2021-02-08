use std::collections::HashMap;
use crate::lang_core::interp::{VarValues, Gc, VarRefType};
use std::cell::RefCell;

mod boolean;
mod math;

macro_rules! add_func {
    ($vars:expr, $func:expr, $($names:expr),+) => {
        {
            let val = VarRefType::Value(Gc::new(RefCell::new(VarValues::RustFunc($func))));
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
    add_func!(vars, boolean::eq_func, "eq");
    add_func!(vars, boolean::ne_func, "ne");
    add_func!(vars, math::add_func, "add");
}