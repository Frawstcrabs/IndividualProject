use crate::lang_core::parse::{AST, VarAccess, AccessorType};

#[derive(Debug, Clone)]
pub enum Instruction {
    PUSHSTR(String),
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
    STARTCATCH(usize),
    ENDCATCH,
    THROWVAL,
    END,
}

fn ast_var_access(prog: &mut Vec<Instruction>, funcs: &mut Vec<(usize, Vec<Instruction>)>, var: &VarAccess) {
    match &var.value[..] {
        [AST::String(s)] => {
            prog.push(Instruction::PUSHSTR(s.to_owned()));
            prog.push(Instruction::GETVAR);
        },
        _ => {
            ast_vec_bytecode(prog, funcs, &var.value);
        },
    }
    for (t, name) in &var.accessors {
        ast_vec_bytecode(prog, funcs, name);
        match t {
            AccessorType::Index => {
                prog.push(Instruction::GETINDEX);
            },
            AccessorType::Attr => {
                prog.push(Instruction::GETATTR);
            },
        }
    }
}

fn ast_bytecode(prog: &mut Vec<Instruction>, funcs: &mut Vec<(usize, Vec<Instruction>)>, ast: &AST, stackvals: &mut usize) {
    match ast {
        AST::String(s) => {
            prog.push(Instruction::PUSHSTR(s.to_owned()));
            *stackvals += 1;
        },
        AST::Function(var, args) => {
            match (&var.value[..], &var.accessors[..]) {
                ([AST::String(s)], []) => match &s[..] {
                    "if" => {
                        assert!(args.len() >= 2);
                        *stackvals += 1;
                        let mut i = 0;
                        let mut end_jumps = Vec::new();
                        let mut prev_jump: usize;
                        while i < args.len() {
                            ast_vec_bytecode(prog, funcs, &args[i]);
                            if i == args.len() - 1 {
                                // else branch, break to avoid an unnecessary jump
                                break;
                            }
                            prev_jump = prog.len();
                            prog.push(Instruction::IFFALSE(0));
                            i += 1;
                            ast_vec_bytecode(prog, funcs, &args[i]);

                            let current_len = prog.len();
                            end_jumps.push(current_len);
                            prog.push(Instruction::GOTO(0));

                            // correct above cond jump to point past this branch
                            match &mut prog[prev_jump] {
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
                            prog.push(Instruction::PUSHNIL);
                        }
                        // correct end jumps to point past all the compiled branches
                        let current_len = prog.len();
                        for inst in end_jumps {
                            match &mut prog[inst] {
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
                        *stackvals += 1;
                        ast_compile_function(prog, funcs, args);
                    },
                    "list" => {
                        for v in args {
                            ast_vec_bytecode(prog, funcs, v);
                        }
                        prog.push(Instruction::CREATELIST(args.len()));
                        *stackvals += 1;
                    },
                    "nonlocal" => {
                        // TODO: compile this only inside function bodies
                        assert!(args.len() == 1);
                        ast_vec_bytecode(prog, funcs, &args[0]);
                        prog.push(Instruction::SETNONLOCAL);
                    },
                    "throw" => {
                        assert!(args.len() == 1);
                        ast_vec_bytecode(prog, funcs, &args[0]);
                        prog.push(Instruction::THROWVAL);
                    },
                    "catch" => {
                        assert!(args.len() == 1);
                        let startcatch_index = prog.len();
                        prog.push(Instruction::STARTCATCH(0));
                        ast_vec_bytecode(prog, funcs, &args[0]);
                        prog.push(Instruction::ENDCATCH);
                        let current_len = prog.len();
                        match &mut prog[startcatch_index] {
                            Instruction::STARTCATCH(loc) => {
                                *loc = current_len;
                            }
                            _ => unreachable!()
                        }
                    },
                    _ => {
                        *stackvals += 1;
                        ast_function_call(prog, funcs, var, args);
                    },
                },
                _ => {
                    *stackvals += 1;
                    ast_function_call(prog, funcs, var, args);
                },
            }
        },
        AST::SetVar(var, val) => {
            match (&var.value[..], &var.accessors[..]) {
                ([AST::String(s)], []) => {
                    prog.push(Instruction::PUSHSTR(s.to_owned()));
                    ast_vec_bytecode(prog, funcs, val);
                    prog.push(Instruction::SETVAR);
                },
                ([AST::String(s)], _) => {
                    prog.push(Instruction::PUSHSTR(s.to_owned()));
                    prog.push(Instruction::GETVAR);
                    for (t, name) in &var.accessors[..var.accessors.len()-1] {
                        ast_vec_bytecode(prog, funcs, name);
                        match t {
                            AccessorType::Index => {
                                prog.push(Instruction::GETINDEX);
                            },
                            AccessorType::Attr => {
                                prog.push(Instruction::GETATTR);
                            },
                        }
                    }
                    let (t, name) = var.accessors.last().unwrap();
                    ast_vec_bytecode(prog, funcs, name);
                    ast_vec_bytecode(prog, funcs, val);
                    match t {
                        AccessorType::Index => {
                            prog.push(Instruction::SETINDEX);
                        },
                        AccessorType::Attr => {
                            prog.push(Instruction::SETATTR);
                        },
                    }
                },
                (_, []) => {
                    ast_vec_bytecode(prog, funcs, &var.value);
                    ast_vec_bytecode(prog, funcs, val);
                    prog.push(Instruction::SETVAR);
                }
                _ => {
                    ast_vec_bytecode(prog, funcs, &var.value);
                    for (t, name) in &var.accessors[..var.accessors.len()-1] {
                        ast_vec_bytecode(prog, funcs, name);
                        match t {
                            AccessorType::Index => {
                                prog.push(Instruction::GETINDEX);
                            },
                            AccessorType::Attr => {
                                prog.push(Instruction::GETATTR);
                            },
                        }
                    }
                    let (t, name) = var.accessors.last().unwrap();
                    ast_vec_bytecode(prog, funcs, name);
                    ast_vec_bytecode(prog, funcs, val);
                    match t {
                        AccessorType::Index => {
                            prog.push(Instruction::SETINDEX);
                        },
                        AccessorType::Attr => {
                            prog.push(Instruction::SETATTR);
                        },
                    }
                },
            }
        },
        AST::DelVar(var) => {
            match (&var.value[..], &var.accessors[..]) {
                ([AST::String(s)], []) => {
                    prog.push(Instruction::PUSHSTR(s.to_owned()));
                    prog.push(Instruction::DELVAR);
                },
                ([AST::String(s)], _) => {
                    prog.push(Instruction::PUSHSTR(s.to_owned()));
                    prog.push(Instruction::GETVAR);
                    for (t, name) in &var.accessors[..var.accessors.len()-1] {
                        ast_vec_bytecode(prog, funcs, name);
                        match t {
                            AccessorType::Index => {
                                prog.push(Instruction::GETINDEX);
                            },
                            AccessorType::Attr => {
                                prog.push(Instruction::GETATTR);
                            },
                        }
                    }
                    let (t, name) = var.accessors.last().unwrap();
                    ast_vec_bytecode(prog, funcs, name);
                    match t {
                        AccessorType::Index => {
                            prog.push(Instruction::DELINDEX);
                        },
                        AccessorType::Attr => {
                            prog.push(Instruction::DELATTR);
                        },
                    }
                },
                (_, []) => {
                    panic!("invalid {del;} call");
                }
                _ => {
                    ast_vec_bytecode(prog, funcs, &var.value);
                    for (t, name) in &var.accessors[..var.accessors.len()-1] {
                        ast_vec_bytecode(prog, funcs, name);
                        match t {
                            AccessorType::Index => {
                                prog.push(Instruction::GETINDEX);
                            },
                            AccessorType::Attr => {
                                prog.push(Instruction::GETATTR);
                            },
                        }
                    }
                    let (t, name) = var.accessors.last().unwrap();
                    ast_vec_bytecode(prog, funcs, name);
                    match t {
                        AccessorType::Index => {
                            prog.push(Instruction::DELINDEX);
                        },
                        AccessorType::Attr => {
                            prog.push(Instruction::DELATTR);
                        },
                    }
                },
            }
        },
        AST::Variable(var, _args) => {
            ast_var_access(prog, funcs, var);
            *stackvals += 1;
        },
    }
}

fn ast_compile_function(prog: &mut Vec<Instruction>, funcs: &mut Vec<(usize, Vec<Instruction>)>, args: &[Vec<AST>]) {
    let mut arg_names = Vec::with_capacity(args.len() - 1);
    for arg in &args[..args.len() - 1] {
        match &arg[..] {
            [AST::String(s)] => {
                arg_names.push(s.to_owned());
            },
            _ => {
                panic!("param name of lambda was not literal string");
            }
        }
    }
    let mut body_code = Vec::new();
    let mut inner_funcs = Vec::new();
    ast_vec_bytecode(&mut body_code, &mut inner_funcs, &args[args.len() - 1]);
    body_code.push(Instruction::END);
    ast_link_functions(&mut body_code, inner_funcs);
    let current_len = prog.len();
    prog.push(Instruction::CREATEFUNC(arg_names, 0, 0));
    funcs.push((current_len, body_code));
}

fn ast_link_functions(prog: &mut Vec<Instruction>, funcs: Vec<(usize, Vec<Instruction>)>) {
    for (func_offset, inst) in funcs {
        let current_len = prog.len();
        match &mut prog[func_offset] {
            Instruction::CREATEFUNC(_, offset, size) => {
                *offset = current_len;
                *size = inst.len();
            },
            _ => unreachable!(),
        }
        prog.extend(inst);
    }
}

fn ast_function_call(
    prog: &mut Vec<Instruction>,
    funcs: &mut Vec<(usize, Vec<Instruction>)>,
    var: &VarAccess,
    args: &[Vec<AST>])
{
    ast_var_access(prog, funcs, var);
    for ast in args {
        ast_vec_bytecode(prog, funcs, ast);
    }
    prog.push(Instruction::CALLFUNC(args.len()));
}

fn ast_vec_bytecode(prog: &mut Vec<Instruction>, funcs: &mut Vec<(usize, Vec<Instruction>)>, astlist: &[AST]) {
    let mut stack_vals = 0;
    for ast in astlist {
        ast_bytecode(prog, funcs, ast, &mut stack_vals);
    }
    match stack_vals {
        0 => {
            // push dummy value
            prog.push(Instruction::PUSHNIL);
        },
        1 => {
            // single item remaining already
        },
        _ => {
            // concat values to a single item
            prog.push(Instruction::CONCAT(stack_vals));
        },
    }
}

pub fn generate_bytecode(ast: &[AST]) -> Vec<Instruction> {
    let mut prog = Vec::new();

    let mut funcs = Vec::new();
    ast_vec_bytecode(&mut prog, &mut funcs, ast);
    prog.push(Instruction::END);
    ast_link_functions(&mut prog, funcs);

    return prog;
}