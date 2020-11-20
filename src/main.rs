mod lang_core;

use lang_core::{parse, bytecode, interp};

fn main() {
    let input = "{set:a;{if:1;b;0;d;1;f;g;};}{a}";
    println!("{:?}", input);
    let (_, ast) = parse::parse_base(input).unwrap();
    let mut ctx = interp::Context::new();
    let program = bytecode::generate_bytecode(&ast);
    for (inst, i) in program.instructions.iter().zip(0..) {
        println!("{:<2} - {:?}", i, inst);
    }
    ctx.interpret(&program);
    println!("stack: {:?}", ctx.stack);
}