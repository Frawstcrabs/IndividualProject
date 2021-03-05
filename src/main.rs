mod lang_core;
mod builtins;

use lang_core::{parse, bytecode, interp::{self, LangError, StdOutOutput}};

use libgc::{GcAllocator};

#[global_allocator]
static ALLOCATOR: GcAllocator = GcAllocator;

fn main() {
    let input = "
{!>oneline}

{set:height:50;}
{set:width:200;}
{set:iterations:30;}
{set:chars: ABCDEFGHIJKLMNOPQRSTUVWXYZ ;}
{func:{min:x:y;}:
    {if:{le:{x}:{y};}:
        {x}
    :
        {y}
    ;}
;}
{set:char_max:{min:{sub:{chars.length}:1;}:{iterations};};}
{set:transx:{fdiv:4:{width};};}
{set:transy:{fdiv:2:{height};};}
{func:{m:xcoord:ycoord;}:
    {set:x0:{sub:{mul:{xcoord}:{transx};}:2.5;};}
    {set:y0:{sub:{mul:{ycoord}:{transy};}:1;};}
    {set:x2:0;}
    {set:y2:0;}
    {set:w:0;}
    {set:i:0;}
    {while:{and:{le:{add:{x2}:{y2};}:4;}:{le:{i}:{char_max};};}:
        {set:x:{add:{sub:{x2}:{y2};}:{x0};};}
        {set:y:{sub:{add:{w}:{y0};}:{add:{x2}:{y2};};};}
        {set:x2:{mul:{x}:{x};};}
        {set:y2:{mul:{y}:{y};};}
        {set:wt:{add:{x}:{y};};}
        {set:w:{mul:{wt}:{wt};};}
        {set:i:{add:{i}:1;};}
    ;}
    {chars[{min:{i}:{char_max};}]}
;}

{for:ycoord:0:{height}:
    {for:xcoord:0:{width}:
        {m:{xcoord}:{ycoord};}
    ;}\\n
;}
    ";
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
    // /*
    let mut ctx = interp::Context::new();
    let ret = ctx.interpret(&program, &mut StdOutOutput{});
    //println!("stack: {:?}", ctx.stack);
    match ret {
        Ok(_) => {}
        Err(LangError::Throw(v)) => {
            println!("Err: {:?}", v.borrow().to_string());
        }
        Err(LangError::CatchUnwind(_)) => {
            panic!("catchunwind escaped interpreter");
        }
    }
    // */
}