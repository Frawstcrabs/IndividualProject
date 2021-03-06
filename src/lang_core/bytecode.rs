#![allow(unreachable_patterns)]

use crate::lang_core::parse::{AST, VarAccess, Accessor};
use std::mem;

#[derive(Debug, Clone)]
pub enum Instruction {
    PUSHSTR(String),
    PUSHASTSTR(String, Option<f64>),
    PUSHNIL,
    PUSHNUM(f64),
    OUTPUTSTR(String, Option<f64>),
    OUTPUTVAL,
    IFFALSE(usize),
    GOTO(usize),
    CONCAT(usize),
    DROP(usize),
    CREATEFUNC(Vec<String>, usize, usize),
    CALLFUNC(usize, bool),
    CREATELIST(usize),
    CREATEMAP(usize),
    GETVAR(String),
    GETINDEX,
    GETATTR,
    SETVAR(String),
    SETINDEX,
    SETATTR,
    DELVAR(String),
    DELINDEX,
    DELATTR,
    SETNONLOCAL(String),
    WHILESTART,
    FORSTART(String),
    FORTEST(usize),
    FORITER,
    FOREACHSTART(String),
    FOREACHITER(usize),
    LOOPINCR,
    LOOPEND(bool),
    STARTCATCH(usize),
    ENDCATCH,
    UNWINDCATCH(usize),
    THROWVAL,
    END,
}

#[derive(Debug)]
enum ValStatus {
    Temp,
    Returned,
}

#[derive(Debug)]
struct LoopJumps {
    breaks: Vec<usize>,
    continues: Vec<usize>,
    catch_count: usize,
    val_counts: Vec<(ValStatus, usize, usize)>,
}

#[derive(Debug)]
struct CompilerCtx {
    prog: Vec<Instruction>,
    funcs: Vec<(usize, Vec<Instruction>)>,
    current_loop: Option<LoopJumps>,
    in_function: bool,
}

impl CompilerCtx {
    #[inline]
    fn set_block_args(&mut self, amount: usize) {
        if let Some(cur_loop) = &mut self.current_loop {
            if let Some((_, _, n)) = &mut cur_loop.val_counts.last_mut() {
                *n = amount;
            }
        }
    }
    #[inline]
    fn inc_catch_count(&mut self) {
        if let Some(cur_loop) = &mut self.current_loop {
            cur_loop.catch_count += 1;
        }
    }
    #[inline]
    fn dec_catch_count(&mut self) {
        if let Some(cur_loop) = &mut self.current_loop {
            cur_loop.catch_count -= 1;
        }
    }
}

#[derive(Debug)]
enum InternalASTErrors {
    LoopJumpCutoff,
    InvalidArgCount(String, usize),
    NonlocalInGlobalScope,
    InvalidIdentifier(String),
    ContinueOutsideOfLoop,
    BreakOutsideOfLoop,
    CannotSetFunctionCall,
    EmptySetCall,
    CannotDelFunctionCall,
    EmptyDelCall
}

#[derive(Debug)]
pub enum ASTErrors {
    InvalidArgCount(String, usize),
    NonlocalInGlobalScope,
    InvalidIdentifier(String),
    ContinueOutsideOfLoop,
    BreakOutsideOfLoop,
    CannotSetFunctionCall,
    EmptySetCall,
    CannotDelFunctionCall,
    EmptyDelCall
}

fn ast_accessor_bytecode(ctx: &mut CompilerCtx, accessor: &Accessor) -> Result<(), InternalASTErrors> {
    match accessor {
        Accessor::Index(arg) => {
            ast_vec_bytecode(ctx, arg, ValStatus::Temp, false, false)?;
            ctx.prog.push(Instruction::GETINDEX);
        },
        Accessor::Attr(arg) => {
            ast_vec_bytecode(ctx, arg, ValStatus::Temp, false, false)?;
            ctx.prog.push(Instruction::GETATTR);
        },
        Accessor::Call(args) => {
            for arg in args {
                ast_vec_bytecode(ctx, arg, ValStatus::Temp, true, false)?;
            }
            ctx.set_block_args(1);
            ctx.prog.push(Instruction::CALLFUNC(args.len(), false));
        },
    }
    Ok(())
}

fn ast_var_access(ctx: &mut CompilerCtx, var: &VarAccess, direct_output: bool) -> Result<(), InternalASTErrors> {
    match &var.value[..] {
        [AST::String(s, _)] => {
            ctx.prog.push(Instruction::GETVAR(s.to_owned()));
            // have to increment it manually, as ast_vec_bytecode
            // won't register it in time
            ctx.set_block_args(1);
        },
        _ => {
            ast_vec_bytecode(ctx, &var.value, ValStatus::Temp, true, false)?;
        },
    }
    for accessor in &var.accessors {
        ast_accessor_bytecode(ctx, accessor)?;
    }
    match (ctx.prog.last_mut(), direct_output) {
        (Some(Instruction::CALLFUNC(_, output)), true) => {
            // CALLFUNC with direct output enabled automatically outputs its vals,
            // so no OUTPUTVAL instruction is needed
            *output = true;
        }
        (_, true) => {
            ctx.prog.push(Instruction::OUTPUTVAL)
        }
        _ => {}
    }
    Ok(())
}

fn count_stack_vals(counts: &Vec<(ValStatus, usize, usize)>) -> (usize, usize) {
    counts.iter()
        .fold((0, 0), |mut acc, x| {
            match x {
                (ValStatus::Temp, block_temps, arg_temps) => {
                    // values to drop
                    acc.0 += arg_temps + block_temps;
                }
                (ValStatus::Returned, block_temps, arg_temps) => {
                    // values to concat
                    acc.0 += arg_temps;
                    acc.1 += block_temps;
                }
            }
            acc
        })
}

fn ast_bytecode(ctx: &mut CompilerCtx, ast: &AST, direct_output: bool) -> Result<bool, InternalASTErrors> {
    //println!("ast_bytecode\n  {:?}\n  {:?}", ast, ctx.current_loop);
    match ast {
        AST::String(s, v) => {
            ctx.prog.push(match direct_output {
                true => Instruction::OUTPUTSTR(s.to_owned(), *v),
                false => Instruction::PUSHASTSTR(s.to_owned(), *v),
            });
            Ok(true)
        },
        AST::Variable(var) => {
            match (&var.value[..], &var.accessors[..]) {
                ([AST::String(s, _)], [Accessor::Call(args)]) => match &s[..] {
                    "if" => {
                        if args.len() < 2 {
                            return Err(InternalASTErrors::InvalidArgCount(String::from("if"), args.len()));
                        }
                        let mut i = 0;
                        let mut end_jumps = Vec::new();
                        let mut prev_jump: usize;
                        while i < args.len() - 1 {
                            ast_vec_bytecode(ctx, &args[i], ValStatus::Temp, false, false)?;
                            prev_jump = ctx.prog.len();
                            ctx.prog.push(Instruction::IFFALSE(0));
                            i += 1;
                            match ast_vec_bytecode(ctx, &args[i], ValStatus::Returned, false, direct_output) {
                                Ok(_) | Err(InternalASTErrors::LoopJumpCutoff) => {},
                                Err(v) => return Err(v),
                            }

                            let current_len = ctx.prog.len();
                            end_jumps.push(current_len);
                            ctx.prog.push(Instruction::GOTO(0));

                            // correct above cond jump to point past this branch
                            match &mut ctx.prog[prev_jump] {
                                Instruction::IFFALSE(p) => {
                                    // add one to skip the above jmp to end
                                    *p = current_len+1;
                                }
                                _ => unreachable!()
                            }
                            i += 1;
                        }
                        if args.len() % 2 == 0 {
                            // no else branch given, add a nil for a placeholder
                            ctx.prog.push(Instruction::PUSHNIL);
                        } else {
                            match ast_vec_bytecode(ctx, args.last().unwrap(), ValStatus::Returned, false, direct_output) {
                                Ok(_) | Err(InternalASTErrors::LoopJumpCutoff) => {},
                                Err(v) => return Err(v),
                            }
                        }
                        // correct end jumps to point past all the compiled branches
                        let current_len = ctx.prog.len();
                        for inst in end_jumps {
                            match &mut ctx.prog[inst] {
                                Instruction::GOTO(p) => {
                                    *p = current_len;
                                }
                                _ => unreachable!()
                            }
                        }
                        Ok(true)
                    },
                    "lambda" => {
                        if args.len() == 0 {
                            return Err(InternalASTErrors::InvalidArgCount(String::from("lambda"), args.len()));
                        }
                        ast_compile_function(ctx, args)?;
                        if direct_output {
                            ctx.prog.push(Instruction::OUTPUTVAL);
                        }
                        Ok(true)
                    },
                    "list" => {
                        for v in args {
                            ast_vec_bytecode(ctx, v, ValStatus::Temp, true, false)?;
                        }
                        ctx.prog.push(Instruction::CREATELIST(args.len()));
                        if direct_output {
                            ctx.prog.push(Instruction::OUTPUTVAL);
                        }
                        Ok(true)
                    },
                    "map" => {
                        if args.len() % 2 != 0 {
                            return Err(InternalASTErrors::InvalidArgCount(String::from("map"), args.len()));
                        }
                        for v in args {
                            ast_vec_bytecode(ctx, v, ValStatus::Temp, true, false)?;
                        }
                        ctx.prog.push(Instruction::CREATEMAP(args.len()));
                        if direct_output {
                            ctx.prog.push(Instruction::OUTPUTVAL);
                        }
                        Ok(true)
                    },
                    "nonlocal" => {
                        if args.len() != 1 {
                            return Err(InternalASTErrors::InvalidArgCount(String::from("nonlocal"), args.len()));
                        }
                        if !ctx.in_function {
                            return Err(InternalASTErrors::NonlocalInGlobalScope);
                        }
                        match &args[0][..] {
                            [AST::String(s, _)] => {
                                ctx.prog.push(Instruction::SETNONLOCAL(s.to_owned()));
                            },
                            _ => {
                                return Err(InternalASTErrors::InvalidIdentifier(String::from("nonlocal")));
                            }
                        }
                        Ok(false)
                    },
                    "throw" => {
                        if args.len() != 1 {
                            return Err(InternalASTErrors::InvalidArgCount(String::from("throw"), args.len()));
                        }
                        ast_vec_bytecode(ctx, &args[0], ValStatus::Temp, true, false)?;
                        ctx.prog.push(Instruction::THROWVAL);
                        Ok(false)
                    },
                    "catch" => {
                        if args.len() != 1 {
                            return Err(InternalASTErrors::InvalidArgCount(String::from("catch"), args.len()));
                        }
                        let startcatch_index = ctx.prog.len();
                        ctx.prog.push(Instruction::STARTCATCH(0));
                        ctx.inc_catch_count();
                        match ast_vec_bytecode(ctx, &args[0], ValStatus::Temp, true, false) {
                            Ok(_) => {
                                ctx.dec_catch_count();
                            }
                            Err(v) => {
                                ctx.dec_catch_count();
                                return Err(v);
                            }
                        }
                        ctx.prog.push(Instruction::ENDCATCH);
                        let current_len = ctx.prog.len();
                        match &mut ctx.prog[startcatch_index] {
                            Instruction::STARTCATCH(loc) => {
                                *loc = current_len;
                            }
                            _ => unreachable!()
                        }
                        if direct_output {
                            ctx.prog.push(Instruction::OUTPUTVAL);
                        }
                        Ok(true)
                    },
                    "void" => {
                        if args.len() != 1 {
                            return Err(InternalASTErrors::InvalidArgCount(String::from("void"), args.len()));
                        }
                        ast_vec_bytecode(ctx, &args[0], ValStatus::Temp, false, false)?;
                        ctx.prog.push(Instruction::DROP(1));
                        Ok(false)
                    },
                    "while" => {
                        if args.len() != 2 {
                            return Err(InternalASTErrors::InvalidArgCount(String::from("while"), args.len()));
                        }
                        ctx.prog.push(Instruction::WHILESTART);
                        let test_start = ctx.prog.len();
                        // this value is technically outside of the while body, so
                        // it should use the outside loop's continue or break if found
                        ast_vec_bytecode(ctx, &args[0], ValStatus::Temp, false, false)?;

                        let outer_loop = mem::replace(
                            &mut ctx.current_loop,
                            Some(LoopJumps {
                                breaks: Vec::new(),
                                continues: Vec::new(),
                                catch_count: 0,
                                val_counts: Vec::new(),
                            })
                        );

                        let false_jump = ctx.prog.len();
                        ctx.prog.push(Instruction::IFFALSE(0));
                        match ast_vec_bytecode(ctx, &args[1], ValStatus::Returned, false, direct_output) {
                            Ok(_) | Err(InternalASTErrors::LoopJumpCutoff) => {},
                            Err(v) => return Err(v),
                        }
                        let continue_jump = ctx.prog.len();
                        ctx.prog.push(Instruction::LOOPINCR);
                        ctx.prog.push(Instruction::GOTO(test_start));
                        let loop_end = ctx.prog.len();
                        ctx.prog.push(Instruction::LOOPINCR);
                        match &mut ctx.prog[false_jump] {
                            Instruction::IFFALSE(ptr) => {
                                *ptr = loop_end+1;
                            },
                            _ => unreachable!(),
                        }
                        let jump_data = mem::replace(
                            &mut ctx.current_loop,
                            outer_loop
                        );
                        if let Some(LoopJumps{breaks, continues, ..}) = jump_data {
                            for index in breaks {
                                match &mut ctx.prog[index] {
                                    Instruction::GOTO(ptr) => {
                                        *ptr = loop_end;
                                    },
                                    _ => unreachable!(),
                                }
                            }
                            for index in continues {
                                match &mut ctx.prog[index] {
                                    Instruction::GOTO(ptr) => {
                                        *ptr = continue_jump;
                                    },
                                    _ => unreachable!(),
                                }
                            }
                        } else {
                            panic!("INTERNAL ERROR: while loop data overwritten inside loop")
                        }
                        ctx.prog.push(Instruction::LOOPEND(!direct_output));
                        Ok(true)
                    },
                    "for" => {
                        if args.len() < 3 || args.len() > 5 {
                            return Err(InternalASTErrors::InvalidArgCount(String::from("for"), args.len()));
                        }
                        let ident = match &args[0][..] {
                            [AST::String(ident, _)] => ident.clone(),
                            _ => {
                                return Err(InternalASTErrors::InvalidIdentifier(String::from("for")));
                            }
                        };
                        ast_vec_bytecode(ctx, &args[0], ValStatus::Temp, true, false)?;
                        match args.len() {
                            3 => {
                                ctx.prog.push(Instruction::PUSHNUM(0.0));
                                ast_vec_bytecode(ctx, &args[1], ValStatus::Temp, true, false)?;
                                ctx.prog.push(Instruction::PUSHNUM(1.0));
                            },
                            4 => {
                                ast_vec_bytecode(ctx, &args[1], ValStatus::Temp, true, false)?;
                                ast_vec_bytecode(ctx, &args[2], ValStatus::Temp, true, false)?;
                                ctx.prog.push(Instruction::PUSHNUM(1.0));
                            },
                            5 => {
                                ast_vec_bytecode(ctx, &args[1], ValStatus::Temp, true, false)?;
                                ast_vec_bytecode(ctx, &args[2], ValStatus::Temp, true, false)?;
                                ast_vec_bytecode(ctx, &args[3], ValStatus::Temp, true, false)?;
                            },
                            _ => unreachable!(),
                        }
                        ctx.set_block_args(0);
                        ctx.prog.push(Instruction::FORSTART(ident));
                        let test_start = ctx.prog.len();
                        ctx.prog.push(Instruction::FORTEST(0));

                        let outer_loop = mem::replace(
                            &mut ctx.current_loop,
                            Some(LoopJumps {
                                breaks: Vec::new(),
                                continues: Vec::new(),
                                catch_count: 0,
                                val_counts: Vec::new(),
                            })
                        );
                        match ast_vec_bytecode(ctx, args.last().unwrap(), ValStatus::Returned, false, direct_output) {
                            Ok(_) | Err(InternalASTErrors::LoopJumpCutoff) => {},
                            Err(v) => return Err(v),
                        }
                        let continue_jump = ctx.prog.len();
                        ctx.prog.push(Instruction::FORITER);
                        ctx.prog.push(Instruction::LOOPINCR);
                        ctx.prog.push(Instruction::GOTO(test_start));
                        let loop_end = ctx.prog.len();
                        ctx.prog.push(Instruction::LOOPINCR);
                        match &mut ctx.prog[test_start] {
                            Instruction::FORTEST(ptr) => {
                                *ptr = loop_end+1;
                            },
                            _ => unreachable!(),
                        }
                        let jump_data = mem::replace(
                            &mut ctx.current_loop,
                            outer_loop
                        );
                        if let Some(LoopJumps{breaks, continues, ..}) = jump_data {
                            for index in breaks {
                                match &mut ctx.prog[index] {
                                    Instruction::GOTO(ptr) => {
                                        *ptr = loop_end;
                                    },
                                    _ => unreachable!(),
                                }
                            }
                            for index in continues {
                                match &mut ctx.prog[index] {
                                    Instruction::GOTO(ptr) => {
                                        *ptr = continue_jump;
                                    },
                                    _ => unreachable!(),
                                }
                            }
                        } else {
                            panic!("INTERNAL ERROR: for loop data overwritten inside loop")
                        }
                        ctx.prog.push(Instruction::LOOPEND(!direct_output));
                        Ok(true)
                    }
                    "foreach" => {
                        if args.len() != 3 {
                            return Err(InternalASTErrors::InvalidArgCount(String::from("foreach"), args.len()));
                        }
                        let ident = match &args[0][..] {
                            [AST::String(ident, _)] => ident.clone(),
                            _ => {
                                return Err(InternalASTErrors::InvalidIdentifier(String::from("foreach")));
                            }
                        };
                        ast_vec_bytecode(ctx, &args[1], ValStatus::Temp, true, false)?;
                        ctx.set_block_args(0);
                        ctx.prog.push(Instruction::FOREACHSTART(ident));
                        let test_start = ctx.prog.len();
                        ctx.prog.push(Instruction::FOREACHITER(0));

                        let outer_loop = mem::replace(
                            &mut ctx.current_loop,
                            Some(LoopJumps {
                                breaks: Vec::new(),
                                continues: Vec::new(),
                                catch_count: 0,
                                val_counts: Vec::new(),
                            })
                        );
                        match ast_vec_bytecode(ctx, args.last().unwrap(), ValStatus::Returned, false, direct_output) {
                            Ok(_) | Err(InternalASTErrors::LoopJumpCutoff) => {},
                            Err(v) => return Err(v),
                        }
                        let continue_jump = ctx.prog.len();
                        ctx.prog.push(Instruction::LOOPINCR);
                        ctx.prog.push(Instruction::GOTO(test_start));
                        let loop_end = ctx.prog.len();
                        ctx.prog.push(Instruction::LOOPINCR);
                        match &mut ctx.prog[test_start] {
                            Instruction::FOREACHITER(ptr) => {
                                *ptr = loop_end+1;
                            },
                            _ => unreachable!(),
                        }
                        let jump_data = mem::replace(
                            &mut ctx.current_loop,
                            outer_loop
                        );
                        if let Some(LoopJumps{breaks, continues, ..}) = jump_data {
                            for index in breaks {
                                match &mut ctx.prog[index] {
                                    Instruction::GOTO(ptr) => {
                                        *ptr = loop_end;
                                    },
                                    _ => unreachable!(),
                                }
                            }
                            for index in continues {
                                match &mut ctx.prog[index] {
                                    Instruction::GOTO(ptr) => {
                                        *ptr = continue_jump;
                                    },
                                    _ => unreachable!(),
                                }
                            }
                        } else {
                            panic!("INTERNAL ERROR: foreach loop data overwritten inside loop")
                        }
                        ctx.prog.push(Instruction::LOOPEND(!direct_output));
                        Ok(true)
                    }
                    "continue" => {
                        if !args.is_empty() {
                            return Err(InternalASTErrors::InvalidArgCount(String::from("continue"), args.len()));
                        }
                        if let Some(LoopJumps{continues, val_counts, catch_count, ..}) = &mut ctx.current_loop {
                            let (temp_vals, ret_vals) = count_stack_vals(val_counts);
                            if temp_vals > 0 {
                                ctx.prog.push(Instruction::DROP(temp_vals));
                            }
                            if !direct_output {
                                match ret_vals {
                                    0 => {
                                        ctx.prog.push(Instruction::PUSHNIL);
                                    },
                                    1 => {},
                                    _ => {
                                        ctx.prog.push(Instruction::CONCAT(ret_vals));
                                    }
                                }
                            }
                            if *catch_count > 0 {
                                ctx.prog.push(Instruction::UNWINDCATCH(*catch_count));
                            }
                            continues.push(ctx.prog.len());
                            ctx.prog.push(Instruction::GOTO(0));
                            Err(InternalASTErrors::LoopJumpCutoff)
                        } else {
                            return Err(InternalASTErrors::ContinueOutsideOfLoop);
                        }
                    },
                    "break" => {
                        if !args.is_empty() {
                            return Err(InternalASTErrors::InvalidArgCount(String::from("break"), args.len()));
                        }
                        if let Some(LoopJumps{breaks, val_counts, catch_count, ..}) = &mut ctx.current_loop {
                            let (temp_vals, ret_vals) = count_stack_vals(val_counts);
                            if temp_vals > 0 {
                                ctx.prog.push(Instruction::DROP(temp_vals));
                            }
                            if !direct_output {
                                match ret_vals {
                                    0 => {
                                        ctx.prog.push(Instruction::PUSHNIL);
                                    },
                                    1 => {},
                                    _ => {
                                        ctx.prog.push(Instruction::CONCAT(ret_vals));
                                    }
                                }
                            }
                            if *catch_count > 0 {
                                ctx.prog.push(Instruction::UNWINDCATCH(*catch_count));
                            }
                            breaks.push(ctx.prog.len());
                            ctx.prog.push(Instruction::GOTO(0));
                            Err(InternalASTErrors::LoopJumpCutoff)
                        } else {
                            return Err(InternalASTErrors::BreakOutsideOfLoop);
                        }
                    },
                    _ => {
                        ast_var_access(ctx, var, direct_output)?;
                        Ok(true)
                    },
                },
                _ => {
                    ast_var_access(ctx, var, direct_output)?;
                    Ok(true)
                }
            }
        },
        AST::SetVar(var, val) => {
            match (&var.value[..], &var.accessors[..]) {
                ([AST::String(s, _)], []) => {
                    ast_vec_bytecode(ctx, val, ValStatus::Temp, true, false)?;
                    ctx.prog.push(Instruction::SETVAR(s.to_owned()));
                },
                ([AST::String(s, _)], _) => {
                    ctx.prog.push(Instruction::GETVAR(s.to_owned()));
                    for accessor in &var.accessors[..var.accessors.len()-1] {
                        ast_accessor_bytecode(ctx, accessor)?;
                    }
                    let accessor = var.accessors.last().unwrap();
                    match accessor {
                        Accessor::Index(arg) => {
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true, false)?;
                            ast_vec_bytecode(ctx, val, ValStatus::Temp, true, false)?;
                            ctx.prog.push(Instruction::SETINDEX);
                        },
                        Accessor::Attr(arg) => {
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true, false)?;
                            ast_vec_bytecode(ctx, val, ValStatus::Temp, true, false)?;
                            ctx.prog.push(Instruction::SETATTR);
                        },
                        Accessor::Call(_) => {
                            return Err(InternalASTErrors::CannotSetFunctionCall);
                        },
                    }
                },
                (_, []) => {
                    return Err(InternalASTErrors::EmptySetCall);
                }
                _ => {
                    ast_vec_bytecode(ctx, &var.value, ValStatus::Temp, true, false)?;
                    for accessor in &var.accessors[..var.accessors.len()-1] {
                        ast_accessor_bytecode(ctx, accessor)?;
                    }
                    let accessor = var.accessors.last().unwrap();
                    match accessor {
                        Accessor::Index(arg) => {
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true, false)?;
                            ast_vec_bytecode(ctx, val, ValStatus::Temp, true, false)?;
                            ctx.prog.push(Instruction::SETINDEX);
                        },
                        Accessor::Attr(arg) => {
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true, false)?;
                            ast_vec_bytecode(ctx, val, ValStatus::Temp, true, false)?;
                            ctx.prog.push(Instruction::SETATTR);
                        },
                        Accessor::Call(_) => {
                            return Err(InternalASTErrors::CannotSetFunctionCall);
                        },
                    }
                },
            }
            Ok(false)
        },
        AST::DelVar(var) => {
            match (&var.value[..], &var.accessors[..]) {
                ([AST::String(s, _)], []) => {
                    ctx.prog.push(Instruction::DELVAR(s.to_owned()));
                },
                ([AST::String(s, _)], _) => {
                    ctx.prog.push(Instruction::GETVAR(s.to_owned()));
                    for accessor in &var.accessors[..var.accessors.len()-1] {
                        ast_accessor_bytecode(ctx, accessor)?;
                    }
                    let accessor = var.accessors.last().unwrap();
                    match accessor {
                        Accessor::Index(arg) => {
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true, false)?;
                            ctx.prog.push(Instruction::DELINDEX);
                        },
                        Accessor::Attr(arg) => {
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true, false)?;
                            ctx.prog.push(Instruction::DELATTR);
                        },
                        Accessor::Call(_) => {
                            return Err(InternalASTErrors::CannotDelFunctionCall);
                        },
                    }
                },
                (_, []) => {
                    return Err(InternalASTErrors::EmptyDelCall);
                }
                _ => {
                    ast_vec_bytecode(ctx, &var.value, ValStatus::Temp, true, false)?;
                    for accessor in &var.accessors[..var.accessors.len()-1] {
                        ast_accessor_bytecode(ctx, accessor)?;
                    }
                    let accessor = var.accessors.last().unwrap();
                    match accessor {
                        Accessor::Index(arg) => {
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true, false)?;
                            ctx.prog.push(Instruction::DELINDEX);
                        },
                        Accessor::Attr(arg) => {
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true, false)?;
                            ctx.prog.push(Instruction::DELATTR);
                        },
                        Accessor::Call(_) => {
                            return Err(InternalASTErrors::CannotDelFunctionCall);
                        },
                    }
                },
            }
            Ok(false)
        },
    }
}

fn ast_compile_function(ctx: &mut CompilerCtx, args: &[Vec<AST>]) -> Result<(), InternalASTErrors> {
    let mut arg_names = Vec::with_capacity(args.len() - 1);
    for arg in &args[..args.len() - 1] {
        match &arg[..] {
            [AST::String(s, _)] => {
                arg_names.push(s.to_owned());
            },
            _ => {
                return Err(InternalASTErrors::InvalidIdentifier(String::from("lambda")));
            }
        }
    }

    let mut func_ctx = CompilerCtx {
        prog: Vec::new(),
        funcs: Vec::new(),
        current_loop: None,
        in_function: true,
    };
    match ast_vec_bytecode(&mut func_ctx, &args[args.len() - 1], ValStatus::Returned, true, true) {
        Err(InternalASTErrors::LoopJumpCutoff) => {
            panic!("INTERNAL ERROR: loop jump cutoff leaked out of function body");
        }
        Err(v) => return Err(v),
        Ok(_) => {}
    }
    func_ctx.prog.push(Instruction::END);
    ast_link_functions(&mut func_ctx);
    let current_len = ctx.prog.len();
    ctx.prog.push(Instruction::CREATEFUNC(arg_names, 0, 0));
    ctx.funcs.push((current_len, func_ctx.prog));
    Ok(())
}

fn ast_link_functions(ctx: &mut CompilerCtx) {
    let funcs = mem::take(&mut ctx.funcs);
    for (func_offset, inst) in funcs {
        let current_len = ctx.prog.len();
        match &mut ctx.prog[func_offset] {
            Instruction::CREATEFUNC(_, offset, size) => {
                *offset = current_len;
                *size = inst.len();
            },
            _ => unreachable!(),
        }
        ctx.prog.extend(inst);
    }
}

fn ast_vec_bytecode(ctx: &mut CompilerCtx,
                    astlist: &[AST],
                    status: ValStatus,
                    add_temp: bool,
                    direct_output: bool) -> Result<(), InternalASTErrors> {
    //println!("ast_vec_bytecode\n  {:?}\n  {:?}", astlist, ctx.current_loop);
    if let Some(cur_loop) = &mut ctx.current_loop {
        match cur_loop.val_counts.last() {
            Some((ValStatus::Temp, _, _)) => {
                // cannot have a returned value within temp values
                cur_loop.val_counts.push((ValStatus::Temp, 0, 0));
            }
            Some((ValStatus::Returned, _, _)) | None => {
                cur_loop.val_counts.push((status, 0, 0));
            }
        }
        for ast in astlist {
            match ast_bytecode(ctx, ast, direct_output) {
                Ok(true) => {
                    let mut stack_entry = ctx.current_loop.as_mut().unwrap().val_counts.last_mut().unwrap();
                    stack_entry.1 += 1;
                    stack_entry.2 = 0;
                }
                Ok(false) => {
                    let mut stack_entry = ctx.current_loop.as_mut().unwrap().val_counts.last_mut().unwrap();
                    stack_entry.2 = 0;
                }
                Err(v) => {
                    ctx.current_loop.as_mut().unwrap().val_counts.pop();
                    return Err(v);
                }
            }
        }
        if !direct_output {
            match ctx.current_loop.as_mut().unwrap().val_counts.pop().unwrap().1 {
                0 => {
                    // push dummy value
                    ctx.prog.push(Instruction::PUSHNIL);
                },
                1 => {
                    // single item remaining already
                },
                n => {
                    // concat values to a single item
                    ctx.prog.push(Instruction::CONCAT(n));
                },
            }
        }
        if add_temp {
            ctx.current_loop.as_mut().unwrap().val_counts.last_mut().unwrap().2 += 1;
        }
    } else {
        let mut stack_vals = 0;
        for ast in astlist {
            match ast_bytecode(ctx, ast, direct_output) {
                Ok(true) => {
                    stack_vals += 1;
                }
                Ok(false) => {}
                Err(v) => {
                    return Err(v);
                }
            }
        }
        if !direct_output {
            match stack_vals {
                0 => {
                    // push dummy value
                    ctx.prog.push(Instruction::PUSHNIL);
                },
                1 => {
                    // single item remaining already
                },
                n => {
                    // concat values to a single item
                    ctx.prog.push(Instruction::CONCAT(n));
                },
            }
        }
    }
    //println!("ast_vec_bytecode finished, {:?}", ctx.current_loop);
    Ok(())
}

pub fn generate_bytecode(ast: &[AST]) -> Result<Vec<Instruction>, ASTErrors> {
    let mut ctx = CompilerCtx {
        prog: Vec::new(),
        funcs: Vec::new(),
        current_loop: None,
        in_function: false,
    };
    match ast_vec_bytecode(&mut ctx, ast, ValStatus::Returned, true, true) {
        Ok(_) => {}
        Err(InternalASTErrors::LoopJumpCutoff) => {
            panic!("INTERNAL ERROR: loop jump cutoff leaked out of program");
        }
        Err(InternalASTErrors::InvalidArgCount(n, c)) => {
            return Err(ASTErrors::InvalidArgCount(n, c));
        }
        Err(InternalASTErrors::NonlocalInGlobalScope) => {
            return Err(ASTErrors::NonlocalInGlobalScope);
        }
        Err(InternalASTErrors::InvalidIdentifier(n)) => {
            return Err(ASTErrors::InvalidIdentifier(n));
        }
        Err(InternalASTErrors::ContinueOutsideOfLoop) => {
            return Err(ASTErrors::ContinueOutsideOfLoop);
        }
        Err(InternalASTErrors::BreakOutsideOfLoop) => {
            return Err(ASTErrors::BreakOutsideOfLoop);
        }
        Err(InternalASTErrors::CannotSetFunctionCall) => {
            return Err(ASTErrors::CannotSetFunctionCall);
        }
        Err(InternalASTErrors::EmptySetCall) => {
            return Err(ASTErrors::EmptySetCall);
        }
        Err(InternalASTErrors::CannotDelFunctionCall) => {
            return Err(ASTErrors::CannotDelFunctionCall);
        }
        Err(InternalASTErrors::EmptyDelCall) => {
            return Err(ASTErrors::EmptyDelCall);
        }
    }
    ctx.prog.push(Instruction::END);
    ast_link_functions(&mut ctx);

    return Ok(ctx.prog);
}