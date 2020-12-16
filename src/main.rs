mod lang_core;
mod builtins;

use lang_core::{parse, bytecode, interp::{self, LangError}};

fn main() {
    let input = "
    {!>oneline}
    {set:a:{list;};}
    {func:{call_3:f;}:
        {f:1;}{f:2;}{f:3;}
    ;}
    {call_3:{a.push};}
    {a[0]} {a[2]} {a[1]}";
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