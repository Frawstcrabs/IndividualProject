mod lang_core;

use lang_core::{parse, bytecode, interp};

fn main() {
    let (_, ast) = parse::parse_base("{set:a;b;}text {a}").unwrap();
    let mut ctx = interp::Context::new();
    let program = bytecode::generate_bytecode(&ast);
    println!("{:?}", program.instructions);
    ctx.interpret(&program);
    println!("{:?}", ctx.stack);
}



