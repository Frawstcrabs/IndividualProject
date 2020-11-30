use crate::lang_core::interp::{VarValues, Context, Gc};
use std::cell::RefCell;

pub fn add_func(_ctx: &mut Context, args: Vec<Gc<VarValues>>) -> Gc<VarValues>{
    assert!(!args.is_empty());

    let mut ret = 0.0;

    for arg in args {
        match &*arg.borrow() {
            VarValues::Num(n) => {
                ret += *n;
            },
            VarValues::Str(s) => {
                ret += s.parse::<f64>().unwrap();
            },
            _ => panic!("something that isnt a number was passed"),
        }
    }

    Gc::new(RefCell::new(VarValues::Num(ret)))
}