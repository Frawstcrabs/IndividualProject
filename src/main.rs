mod lang_core;
mod builtins;

use lang_core::{parse, bytecode, interp::{self, LangError, StdOutOutput}};
use libgc::{GcAllocator};
use clap::{App, Arg};
use std::fs;

#[global_allocator]
static ALLOCATOR: GcAllocator = GcAllocator;

fn main() {
    let matches = App::new("project")
        .help("Individual Project\n\
               Language Interpreter v1.0\n\
               Z. Nuccio (k1891842@kcl.ac.uk)\n\
               \n\
               USAGE: project <-c CODE | FILE> [args...]\
               \n\
               Options:\n\
               -h, --help    Prints this message\n\
               -c, --code    Interpret argument as program")
        .arg(Arg::with_name("code")
            .short("c")
            .long("code")
            .takes_value(true))
        .arg(Arg::with_name("args")
            .multiple(true)
            .min_values(0))
        .get_matches();

    let mut args = match matches.values_of("args") {
        Some(iter) => iter.collect(),
        None => Vec::new(),
    };
    let input;

    match matches.value_of("code") {
        None => {
            if args.is_empty() {
                eprintln!("ERROR: no program inputted");
                return;
            }
            let filename = args.remove(0);
            input = fs::read_to_string(filename)
                .expect("ERROR: could not read file");
        }
        Some(name) => {
            input = name.to_owned();
        }
    }
    let args = args.into_iter().map(|s| s.to_owned()).collect();

    let ast = match parse::run_parser(&input) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("ERROR: could not parse program");
            return;
        }
    };
    //println!("ast: {:?}", ast);
    let program = match bytecode::generate_bytecode(&ast) {
        Ok(prog) => prog,
        Err(val) => {
            eprintln!("SYNTAX ERROR: {:?}", val);
            return;
        }
    };
    // for (inst, i) in program.iter().zip(0..) {
    //     println!("{:<2} - {:?}", i, inst);
    // }
    let mut ctx = interp::Context::with_args(args);
    let ret = ctx.interpret(&program, &mut StdOutOutput{});

    match ret {
        Ok(_) => {
            println!();
        }
        Err(LangError::Throw(v)) => {
            println!("{}", v.borrow().to_string());
        }
        Err(LangError::CatchUnwind(_)) => {
            panic!("INTERNAL ERROR: catchunwind escaped interpreter");
        }
    }
}