mod lang_core;
mod builtins;

use lang_core::{parse, bytecode, interp::{self, LangError}};

fn main() {
    let input = "
    {!>oneline}

    {func:example;a;b;c;
        {! comment !}
        {a} {c} {b}
    ;}

    {example:1;2;3;}\\nfoo

    {add:1;2;no;}";
    println!("{:?}", input);
    let ast = match parse::run_parser(input) {
        Ok(v) => v,
        Err(_) => {
            println!("Error parsing code");
            return;
        }
    };
    let mut ctx = interp::Context::new();
    let program = bytecode::generate_bytecode(&ast);
    for (inst, i) in program.iter().zip(0..) {
        println!("{:<2} - {:?}", i, inst);
    }
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