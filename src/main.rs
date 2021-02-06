mod lang_core;
mod builtins;

use lang_core::{parse, bytecode, interp::{self, LangError}};

use libgc::{GcAllocator};

#[global_allocator]
static ALLOCATOR: GcAllocator = GcAllocator;

fn main() {
    let input = "{!>oneline}

    {set:i:0;}
    {while:{ne:{i}:10;}:
        abc
        {set:i:{add:{i}:1;};}
    ;}";
    //println!("{:?}", input);
    let ast = match parse::run_parser(input) {
        Ok(v) => v,
        Err(_) => {
            println!("Error parsing code");
            return;
        }
    };
    //println!("ast: {:?}", ast);
    let program = bytecode::generate_bytecode(&ast);
    for (inst, i) in program.iter().zip(0..) {
        println!("{:<2} - {:?}", i, inst);
    }
    let mut ctx = interp::Context::new();
    let ret = ctx.interpret(&program);
    println!("stack: {:?}", ctx.stack);
    match ret {
        Ok(_) => {
            println!("Ok: {:?}", ctx.stack[0].borrow().to_string());
        }
        Err(LangError::Throw(v)) => {
            println!("Err: {:?}", v.borrow().to_string());
        }
    }
}