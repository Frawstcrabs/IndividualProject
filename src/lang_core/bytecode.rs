use crate::lang_core::parse::{AST, VarAccess, Accessor};
use std::mem;

#[derive(Debug, Clone)]
pub enum Instruction {
    PUSHSTR(String),
    PUSHASTSTR(String, Option<f64>),
    PUSHNIL,
    IFFALSE(usize),
    GOTO(usize),
    CONCAT(usize),
    CREATEFUNC(Vec<String>, usize, usize),
    CALLFUNC(usize),
    CREATELIST(usize),
    GETVAR,
    GETINDEX,
    GETATTR,
    SETVAR,
    SETINDEX,
    SETATTR,
    DELVAR,
    DELINDEX,
    DELATTR,
    SETNONLOCAL,
    WHILESTART,
    WHILEINCR,
    WHILEEND,
    STARTCATCH(usize),
    ENDCATCH,
    THROWVAL,
    END,
}

struct CompilerCtx {
    prog: Vec<Instruction>,
    funcs: Vec<(usize, Vec<Instruction>)>,
}

fn ast_accessor_bytecode(ctx: &mut CompilerCtx, accessor: &Accessor) {
    match accessor {
        Accessor::Index(arg) => {
            ast_vec_bytecode(ctx, arg);
            ctx.prog.push(Instruction::GETINDEX);
        },
        Accessor::Attr(arg) => {
            ast_vec_bytecode(ctx, arg);
            ctx.prog.push(Instruction::GETATTR);
        },
        Accessor::Call(args) => {
            for arg in args {
                ast_vec_bytecode(ctx, arg);
            }
            ctx.prog.push(Instruction::CALLFUNC(args.len()));
        },
    }
}

fn ast_var_access(ctx: &mut CompilerCtx, var: &VarAccess) {
    match &var.value[..] {
        [AST::String(s, v)] => {
            ctx.prog.push(Instruction::PUSHASTSTR(s.to_owned(), *v));
            ctx.prog.push(Instruction::GETVAR);
        },
        _ => {
            ast_vec_bytecode(ctx, &var.value);
        },
    }
    for accessor in &var.accessors {
        ast_accessor_bytecode(ctx, accessor);
    }
}

fn ast_bytecode(ctx: &mut CompilerCtx, ast: &AST, stack_vals: &mut usize) {
    match ast {
        AST::String(s, v) => {
            ctx.prog.push(Instruction::PUSHASTSTR(s.to_owned(), *v));
            *stack_vals += 1;
        },
        AST::Variable(var) => {
            match (&var.value[..], &var.accessors[..]) {
                ([AST::String(s, _)], [Accessor::Call(args)]) => match &s[..] {
                    "if" => {
                        assert!(args.len() >= 2);
                        *stack_vals += 1;
                        let mut i = 0;
                        let mut end_jumps = Vec::new();
                        let mut prev_jump: usize;
                        while i < args.len() {
                            ast_vec_bytecode(ctx, &args[i]);
                            if i == args.len() - 1 {
                                // else branch, break to avoid an unnecessary jump
                                break;
                            }
                            prev_jump = ctx.prog.len();
                            ctx.prog.push(Instruction::IFFALSE(0));
                            i += 1;
                            ast_vec_bytecode(ctx, &args[i]);

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
                    },
                    "lambda" => {
                        assert!(args.len() >= 1);
                        // all args before last are parameters
                        // must be literal strings and not variable/function calls
                        // last arg is the function body
                        *stack_vals += 1;
                        ast_compile_function(ctx, args);
                    },
                    "list" => {
                        for v in args {
                            ast_vec_bytecode(ctx, v);
                        }
                        ctx.prog.push(Instruction::CREATELIST(args.len()));
                        *stack_vals += 1;
                    },
                    "nonlocal" => {
                        // TODO: compile this only inside function bodies
                        assert!(args.len() == 1);
                        ast_vec_bytecode(ctx, &args[0]);
                        ctx.prog.push(Instruction::SETNONLOCAL);
                    },
                    "throw" => {
                        assert!(args.len() == 1);
                        ast_vec_bytecode(ctx, &args[0]);
                        ctx.prog.push(Instruction::THROWVAL);
                    },
                    "catch" => {
                        assert!(args.len() == 1);
                        let startcatch_index = ctx.prog.len();
                        ctx.prog.push(Instruction::STARTCATCH(0));
                        ast_vec_bytecode(ctx, &args[0]);
                        ctx.prog.push(Instruction::ENDCATCH);
                        let current_len = ctx.prog.len();
                        match &mut ctx.prog[startcatch_index] {
                            Instruction::STARTCATCH(loc) => {
                                *loc = current_len;
                            }
                            _ => unreachable!()
                        }
                        *stack_vals += 1;
                    },
                    "while" => {
                        assert!(args.len() == 2);
                        ctx.prog.push(Instruction::WHILESTART);
                        let test_start = ctx.prog.len();
                        ast_vec_bytecode(ctx, &args[0]);
                        let false_jump = ctx.prog.len();
                        ctx.prog.push(Instruction::IFFALSE(0));
                        ast_vec_bytecode(ctx, &args[1]);
                        ctx.prog.push(Instruction::WHILEINCR);
                        ctx.prog.push(Instruction::GOTO(test_start));
                        let loop_end = ctx.prog.len();
                        match &mut ctx.prog[false_jump] {
                            Instruction::IFFALSE(ptr) => {
                                *ptr = loop_end;
                            },
                            _ => unreachable!(),
                        }
                        ctx.prog.push(Instruction::WHILEEND);
                        *stack_vals += 1;
                    },
                    _ => {
                        ast_var_access(ctx, var);
                        *stack_vals += 1;
                    },
                },
                _ => {
                    ast_var_access(ctx, var);
                    *stack_vals += 1;
                }
            }
        },
        AST::SetVar(var, val) => {
            match (&var.value[..], &var.accessors[..]) {
                ([AST::String(s, v)], []) => {
                    ctx.prog.push(Instruction::PUSHASTSTR(s.to_owned(), *v));
                    ast_vec_bytecode(ctx, val);
                    ctx.prog.push(Instruction::SETVAR);
                },
                ([AST::String(s, v)], _) => {
                    ctx.prog.push(Instruction::PUSHASTSTR(s.to_owned(), *v));
                    ctx.prog.push(Instruction::GETVAR);
                    for accessor in &var.accessors[..var.accessors.len()-1] {
                        ast_accessor_bytecode(ctx, accessor);
                    }
                    let accessor = var.accessors.last().unwrap();
                    match accessor {
                        Accessor::Index(arg) => {
                            ast_vec_bytecode(ctx, arg);
                            ast_vec_bytecode(ctx, val);
                            ctx.prog.push(Instruction::SETINDEX);
                        },
                        Accessor::Attr(arg) => {
                            ast_vec_bytecode(ctx, arg);
                            ast_vec_bytecode(ctx, val);
                            ctx.prog.push(Instruction::SETATTR);
                        },
                        Accessor::Call(_) => {
                            panic!("cannot set to function call");
                        },
                    }
                },
                (_, []) => {
                    ast_vec_bytecode(ctx, &var.value);
                    ast_vec_bytecode(ctx, val);
                    ctx.prog.push(Instruction::SETVAR);
                }
                _ => {
                    ast_vec_bytecode(ctx, &var.value);
                    for accessor in &var.accessors[..var.accessors.len()-1] {
                        ast_accessor_bytecode(ctx, accessor);
                    }
                    let accessor = var.accessors.last().unwrap();
                    match accessor {
                        Accessor::Index(arg) => {
                            ast_vec_bytecode(ctx, arg);
                            ast_vec_bytecode(ctx, val);
                            ctx.prog.push(Instruction::SETINDEX);
                        },
                        Accessor::Attr(arg) => {
                            ast_vec_bytecode(ctx, arg);
                            ast_vec_bytecode(ctx, val);
                            ctx.prog.push(Instruction::SETATTR);
                        },
                        Accessor::Call(_) => {
                            panic!("cannot set to function call");
                        },
                    }
                },
            }
        },
        AST::DelVar(var) => {
            match (&var.value[..], &var.accessors[..]) {
                ([AST::String(s, v)], []) => {
                    ctx.prog.push(Instruction::PUSHASTSTR(s.to_owned(), *v));
                    ctx.prog.push(Instruction::DELVAR);
                },
                ([AST::String(s, v)], _) => {
                    ctx.prog.push(Instruction::PUSHASTSTR(s.to_owned(), *v));
                    ctx.prog.push(Instruction::GETVAR);
                    for accessor in &var.accessors[..var.accessors.len()-1] {
                        ast_accessor_bytecode(ctx, accessor);
                    }
                    let accessor = var.accessors.last().unwrap();
                    match accessor {
                        Accessor::Index(arg) => {
                            ast_vec_bytecode(ctx, arg);
                            ctx.prog.push(Instruction::DELINDEX);
                        },
                        Accessor::Attr(arg) => {
                            ast_vec_bytecode(ctx, arg);
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
                    ast_vec_bytecode(ctx, &var.value);
                    for accessor in &var.accessors[..var.accessors.len()-1] {
                        ast_accessor_bytecode(ctx, accessor);
                    }
                    let accessor = var.accessors.last().unwrap();
                    match accessor {
                        Accessor::Index(arg) => {
                            ast_vec_bytecode(ctx, arg);
                            ctx.prog.push(Instruction::DELINDEX);
                        },
                        Accessor::Attr(arg) => {
                            ast_vec_bytecode(ctx, arg);
                            ctx.prog.push(Instruction::DELATTR);
                        },
                        Accessor::Call(_) => {
                            panic!("cannot del function call");
                        },
                    }
                },
            }
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
    };
    ast_vec_bytecode(&mut func_ctx, &args[args.len() - 1]);
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

fn ast_vec_bytecode(ctx: &mut CompilerCtx, astlist: &[AST]) {
    let mut stack_vals = 0;
    for ast in astlist {
        ast_bytecode(ctx, ast, &mut stack_vals);
    }
    match stack_vals {
        0 => {
            // push dummy value
            ctx.prog.push(Instruction::PUSHNIL);
        },
        1 => {
            // single item remaining already
        },
        _ => {
            // concat values to a single item
            ctx.prog.push(Instruction::CONCAT(stack_vals));
        },
    }
}

pub fn generate_bytecode(ast: &[AST]) -> Vec<Instruction> {
    let mut ctx = CompilerCtx {
        prog: Vec::new(),
        funcs: Vec::new(),
    };
    ast_vec_bytecode(&mut ctx, ast);
    ctx.prog.push(Instruction::END);
    ast_link_functions(&mut ctx);

    return ctx.prog;
}