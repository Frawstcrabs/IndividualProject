use crate::lang_core::parse::{AST, VarAccess, Accessor};
use std::mem;

#[derive(Debug, Clone)]
pub enum Instruction {
    PUSHSTR(String),
    PUSHASTSTR(String, Option<f64>),
    PUSHNIL,
    PUSHNUM(f64),
    IFFALSE(usize),
    GOTO(usize),
    CONCAT(usize),
    DROP(usize),
    CREATEFUNC(Vec<String>, usize, usize),
    CALLFUNC(usize),
    CREATELIST(usize),
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
    FORSTART,
    FORTEST(usize),
    FORITER,
    LOOPINCR,
    LOOPEND,
    STARTCATCH(usize),
    ENDCATCH,
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
    fn set_block_args(&mut self, amount: usize) {
        if let Some(cur_loop) = &mut self.current_loop {
            if let Some((_, _, n)) = &mut cur_loop.val_counts.last_mut() {
                *n = amount;
            }
        }
    }
}

#[derive(Debug)]
enum ASTErrors {
    LoopJumpCutoff,
}

fn ast_accessor_bytecode(ctx: &mut CompilerCtx, accessor: &Accessor) -> Result<(), ASTErrors> {
    match accessor {
        Accessor::Index(arg) => {
            ast_vec_bytecode(ctx, arg, ValStatus::Temp, false)?;
            ctx.prog.push(Instruction::GETINDEX);
        },
        Accessor::Attr(arg) => {
            ast_vec_bytecode(ctx, arg, ValStatus::Temp, false)?;
            ctx.prog.push(Instruction::GETATTR);
        },
        Accessor::Call(args) => {
            for arg in args {
                ast_vec_bytecode(ctx, arg, ValStatus::Temp, true)?;
            }
            ctx.set_block_args(1);
            ctx.prog.push(Instruction::CALLFUNC(args.len()));
        },
    }
    Ok(())
}

fn ast_var_access(ctx: &mut CompilerCtx, var: &VarAccess) -> Result<(), ASTErrors> {
    match &var.value[..] {
        [AST::String(s, _)] => {
            ctx.prog.push(Instruction::GETVAR(s.to_owned()));
            // have to increment it manually, as ast_vec_bytecode
            // won't register it in time
            ctx.set_block_args(1);
        },
        _ => {
            ast_vec_bytecode(ctx, &var.value, ValStatus::Temp, true)?;
        },
    }
    for accessor in &var.accessors {
        ast_accessor_bytecode(ctx, accessor)?;
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

fn ast_bytecode(ctx: &mut CompilerCtx, ast: &AST) -> Result<bool, ASTErrors> {
    //println!("ast_bytecode\n  {:?}\n  {:?}", ast, ctx.current_loop);
    match ast {
        AST::String(s, v) => {
            ctx.prog.push(Instruction::PUSHASTSTR(s.to_owned(), *v));
            Ok(true)
        },
        AST::Variable(var) => {
            match (&var.value[..], &var.accessors[..]) {
                ([AST::String(s, _)], [Accessor::Call(args)]) => match &s[..] {
                    "if" => {
                        assert!(args.len() >= 2);
                        let mut i = 0;
                        let mut end_jumps = Vec::new();
                        let mut prev_jump: usize;
                        while i < args.len() - 1 {
                            ast_vec_bytecode(ctx, &args[i], ValStatus::Temp, false)?;
                            prev_jump = ctx.prog.len();
                            ctx.prog.push(Instruction::IFFALSE(0));
                            i += 1;
                            // if blocks should be returned if a continue or break is
                            // reached, so they are considered returned values
                            ast_vec_bytecode(ctx, &args[i], ValStatus::Returned, false);

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
                            ast_vec_bytecode(ctx, args.last().unwrap(), ValStatus::Returned, false);
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
                        assert!(args.len() >= 1);
                        // all args before last are parameters
                        // must be literal strings and not variable/function calls
                        // last arg is the function body
                        ast_compile_function(ctx, args);
                        Ok(true)
                    },
                    "list" => {
                        for v in args {
                            ast_vec_bytecode(ctx, v, ValStatus::Temp, true)?;
                        }
                        ctx.prog.push(Instruction::CREATELIST(args.len()));
                        Ok(true)
                    },
                    "nonlocal" => {
                        assert!(args.len() == 1);
                        if !ctx.in_function {
                            panic!("nonlocal only allowed in functions");
                        }
                        match &args[0][..] {
                            [AST::String(s, _)] => {
                                ctx.prog.push(Instruction::SETNONLOCAL(s.to_owned()));
                            },
                            _ => {
                                panic!("cannot set arbitrary expressions as nonlocal");
                            }
                        }
                        Ok(false)
                    },
                    "throw" => {
                        assert!(args.len() == 1);
                        ast_vec_bytecode(ctx, &args[0], ValStatus::Temp, true)?;
                        ctx.prog.push(Instruction::THROWVAL);
                        Ok(false)
                    },
                    "catch" => {
                        assert!(args.len() == 1);
                        let startcatch_index = ctx.prog.len();
                        ctx.prog.push(Instruction::STARTCATCH(0));
                        ast_vec_bytecode(ctx, &args[0], ValStatus::Temp, true)?;
                        ctx.prog.push(Instruction::ENDCATCH);
                        let current_len = ctx.prog.len();
                        match &mut ctx.prog[startcatch_index] {
                            Instruction::STARTCATCH(loc) => {
                                *loc = current_len;
                            }
                            _ => unreachable!()
                        }
                        Ok(true)
                    },
                    "while" => {
                        assert!(args.len() == 2);
                        ctx.prog.push(Instruction::WHILESTART);
                        let test_start = ctx.prog.len();
                        // this value is technically outside of the while body, so
                        // it should use the outside loop's continue or break if found
                        ast_vec_bytecode(ctx, &args[0], ValStatus::Temp, false)?;

                        let outer_loop = mem::replace(
                            &mut ctx.current_loop,
                            Some(LoopJumps {
                                breaks: Vec::new(),
                                continues: Vec::new(),
                                val_counts: Vec::new(),
                            })
                        );

                        let false_jump = ctx.prog.len();
                        ctx.prog.push(Instruction::IFFALSE(0));
                        // if a continue happens, if bodies can just take it
                        ast_vec_bytecode(ctx, &args[1], ValStatus::Returned, false);
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
                        let while_data = mem::replace(
                            &mut ctx.current_loop,
                            outer_loop
                        );
                        if let Some(LoopJumps{breaks, continues, ..}) = while_data {
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
                            panic!("while loop data overwritten inside loop")
                        }
                        ctx.prog.push(Instruction::LOOPEND);
                        Ok(true)
                    },
                    "for" => {
                        assert!(args.len() >= 3 && args.len() <= 5);
                        ast_vec_bytecode(ctx, &args[0], ValStatus::Temp, true);
                        match args.len() {
                            3 => {
                                ctx.prog.push(Instruction::PUSHNUM(0.0));
                                ast_vec_bytecode(ctx, &args[1], ValStatus::Temp, true);
                                ctx.prog.push(Instruction::PUSHNUM(1.0));
                            },
                            4 => {
                                ast_vec_bytecode(ctx, &args[1], ValStatus::Temp, true);
                                ast_vec_bytecode(ctx, &args[2], ValStatus::Temp, true);
                                ctx.prog.push(Instruction::PUSHNUM(1.0));
                            },
                            5 => {
                                ast_vec_bytecode(ctx, &args[1], ValStatus::Temp, true);
                                ast_vec_bytecode(ctx, &args[2], ValStatus::Temp, true);
                                ast_vec_bytecode(ctx, &args[3], ValStatus::Temp, true);
                            },
                            _ => unreachable!(),
                        }
                        ctx.set_block_args(0);
                        ctx.prog.push(Instruction::FORSTART);
                        let test_start = ctx.prog.len();
                        ctx.prog.push(Instruction::FORTEST(0));

                        let outer_loop = mem::replace(
                            &mut ctx.current_loop,
                            Some(LoopJumps {
                                breaks: Vec::new(),
                                continues: Vec::new(),
                                val_counts: Vec::new(),
                            })
                        );
                        // if a continue happens, if bodies can just take it
                        ast_vec_bytecode(ctx, args.last().unwrap(), ValStatus::Returned, false);
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
                        let while_data = mem::replace(
                            &mut ctx.current_loop,
                            outer_loop
                        );
                        if let Some(LoopJumps{breaks, continues, ..}) = while_data {
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
                            panic!("for loop data overwritten inside loop")
                        }
                        ctx.prog.push(Instruction::LOOPEND);
                        Ok(true)
                    }
                    "continue" => {
                        assert!(args.is_empty());
                        if let Some(LoopJumps{continues, val_counts, ..}) = &mut ctx.current_loop {
                            let (temp_vals, ret_vals) = count_stack_vals(val_counts);
                            println!("continue, {:?}, ({:?}, {:?})", val_counts, temp_vals, ret_vals);
                            if temp_vals > 0 {
                                ctx.prog.push(Instruction::DROP(temp_vals));
                            }
                            match ret_vals {
                                0 => {
                                    ctx.prog.push(Instruction::PUSHNIL);
                                },
                                1 => {

                                },
                                _ => {
                                    ctx.prog.push(Instruction::CONCAT(ret_vals));
                                }
                            }
                            continues.push(ctx.prog.len());
                            ctx.prog.push(Instruction::GOTO(0));
                            Err(ASTErrors::LoopJumpCutoff)
                        } else {
                            panic!("continue used outside of loop");
                        }
                    },
                    "break" => {
                        assert!(args.is_empty());
                        if let Some(LoopJumps{breaks, val_counts, ..}) = &mut ctx.current_loop {
                            let (temp_vals, ret_vals) = count_stack_vals(val_counts);
                            println!("break, {:?}, ({:?}, {:?})", val_counts, temp_vals, ret_vals);
                            if temp_vals > 0 {
                                ctx.prog.push(Instruction::DROP(temp_vals));
                            }
                            match ret_vals {
                                0 => {
                                    ctx.prog.push(Instruction::PUSHNIL);
                                },
                                1 => {

                                },
                                _ => {
                                    ctx.prog.push(Instruction::CONCAT(ret_vals));
                                }
                            }
                            breaks.push(ctx.prog.len());
                            ctx.prog.push(Instruction::GOTO(0));
                            Err(ASTErrors::LoopJumpCutoff)
                        } else {
                            panic!("break used outside of loop");
                        }
                    },
                    _ => {
                        ast_var_access(ctx, var)?;
                        Ok(true)
                    },
                },
                _ => {
                    ast_var_access(ctx, var)?;
                    Ok(true)
                }
            }
        },
        AST::SetVar(var, val) => {
            match (&var.value[..], &var.accessors[..]) {
                ([AST::String(s, _)], []) => {
                    ast_vec_bytecode(ctx, val, ValStatus::Temp, true)?;
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
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true)?;
                            ast_vec_bytecode(ctx, val, ValStatus::Temp, true)?;
                            ctx.prog.push(Instruction::SETINDEX);
                        },
                        Accessor::Attr(arg) => {
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true)?;
                            ast_vec_bytecode(ctx, val, ValStatus::Temp, true)?;
                            ctx.prog.push(Instruction::SETATTR);
                        },
                        Accessor::Call(_) => {
                            panic!("cannot set to function call");
                        },
                    }
                },
                (_, []) => {
                    panic!("cannot set arbitrary values");
                }
                _ => {
                    ast_vec_bytecode(ctx, &var.value, ValStatus::Temp, true)?;
                    for accessor in &var.accessors[..var.accessors.len()-1] {
                        ast_accessor_bytecode(ctx, accessor)?;
                    }
                    let accessor = var.accessors.last().unwrap();
                    match accessor {
                        Accessor::Index(arg) => {
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true)?;
                            ast_vec_bytecode(ctx, val, ValStatus::Temp, true)?;
                            ctx.prog.push(Instruction::SETINDEX);
                        },
                        Accessor::Attr(arg) => {
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true)?;
                            ast_vec_bytecode(ctx, val, ValStatus::Temp, true)?;
                            ctx.prog.push(Instruction::SETATTR);
                        },
                        Accessor::Call(_) => {
                            panic!("cannot set to function call");
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
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true)?;
                            ctx.prog.push(Instruction::DELINDEX);
                        },
                        Accessor::Attr(arg) => {
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true)?;
                            ctx.prog.push(Instruction::DELATTR);
                        },
                        Accessor::Call(_) => {
                            panic!("cannot del function call");
                        },
                    }
                },
                (_, []) => {
                    panic!("invalid {del;} call");
                }
                _ => {
                    ast_vec_bytecode(ctx, &var.value, ValStatus::Temp, true)?;
                    for accessor in &var.accessors[..var.accessors.len()-1] {
                        ast_accessor_bytecode(ctx, accessor)?;
                    }
                    let accessor = var.accessors.last().unwrap();
                    match accessor {
                        Accessor::Index(arg) => {
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true)?;
                            ctx.prog.push(Instruction::DELINDEX);
                        },
                        Accessor::Attr(arg) => {
                            ast_vec_bytecode(ctx, arg, ValStatus::Temp, true)?;
                            ctx.prog.push(Instruction::DELATTR);
                        },
                        Accessor::Call(_) => {
                            panic!("cannot del function call");
                        },
                    }
                },
            }
            Ok(false)
        },
    }
}

fn ast_compile_function(ctx: &mut CompilerCtx, args: &[Vec<AST>]) {
    let mut arg_names = Vec::with_capacity(args.len() - 1);
    for arg in &args[..args.len() - 1] {
        match &arg[..] {
            [AST::String(s, _)] => {
                arg_names.push(s.to_owned());
            },
            _ => {
                panic!("param name of lambda was not literal string");
            }
        }
    }

    let mut func_ctx = CompilerCtx {
        prog: Vec::new(),
        funcs: Vec::new(),
        current_loop: None,
        in_function: true,
    };
    ast_vec_bytecode(&mut func_ctx, &args[args.len() - 1], ValStatus::Returned, true)
        .expect("ASTError leaked outside of compiler");
    func_ctx.prog.push(Instruction::END);
    ast_link_functions(&mut func_ctx);
    let current_len = ctx.prog.len();
    ctx.prog.push(Instruction::CREATEFUNC(arg_names, 0, 0));
    ctx.funcs.push((current_len, func_ctx.prog));
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

fn ast_vec_bytecode(ctx: &mut CompilerCtx, astlist: &[AST], status: ValStatus, add_temp: bool) -> Result<(), ASTErrors> {
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
            match ast_bytecode(ctx, ast) {
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
        if add_temp {
            ctx.current_loop.as_mut().unwrap().val_counts.last_mut().unwrap().2 += 1;
        }
    } else {
        let mut stack_vals = 0;
        for ast in astlist {
            match ast_bytecode(ctx, ast) {
                Ok(true) => {
                    stack_vals += 1;
                }
                Ok(false) => {}
                Err(v) => {
                    return Err(v);
                }
            }
        }
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
    //println!("ast_vec_bytecode finished, {:?}", ctx.current_loop);
    Ok(())
}

pub fn generate_bytecode(ast: &[AST]) -> Vec<Instruction> {
    let mut ctx = CompilerCtx {
        prog: Vec::new(),
        funcs: Vec::new(),
        current_loop: None,
        in_function: false,
    };
    ast_vec_bytecode(&mut ctx, ast, ValStatus::Returned, true)
        .expect("ASTError leaked outside of compiler");
    ctx.prog.push(Instruction::END);
    ast_link_functions(&mut ctx);

    return ctx.prog;
}