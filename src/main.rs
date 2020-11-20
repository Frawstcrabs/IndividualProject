mod lang_core;

use lang_core::{parse, bytecode, interp};

fn main() {
    let input = "{set:a;{if:0;not {!comment!}run;1;is run;1;not run;else;};}{a}}";
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
    for (inst, i) in program.instructions.iter().zip(0..) {
        println!("{:<2} - {:?}", i, inst);
    }
    ctx.interpret(&program);
    println!("stack: {:?}", ctx.stack);
}