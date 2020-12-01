mod lang_core;
mod builtins;

use lang_core::{parse, bytecode, interp};

fn main() {
    let input = "{call:{lambda:a;{if:{a};true;false;};};1;}";
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
    ctx.interpret(&program);
    println!("stack: {:?}", ctx.stack);
}