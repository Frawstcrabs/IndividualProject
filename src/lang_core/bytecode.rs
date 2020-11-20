use crate::lang_core::parse::AST;

#[derive(Debug)]
pub enum Instruction {
    PUSHSTR(String),
    PUSHNIL,
    IFFALSE(usize),
    GOTO(usize),
    CONCAT(usize),
    SETVAR,
    DEREFVAR,
    END,
}

pub struct Program {
    pub instructions: Vec<Instruction>
}

fn ast_bytecode(prog: &mut Program, ast: &AST, stackvals: &mut usize) {
    match ast {
        AST::String(s) => {
            prog.instructions.push(Instruction::PUSHSTR(s.to_owned()));
            *stackvals += 1;
        },
        AST::Function(args) => {
            let name = &args[0];
            match &name[..] {
                [AST::String(s)] => match &s[..] {
                    "set" => {
                        assert_eq!(args.len(), 3);
                        ast_vec_bytecode(prog, &args[1]);
                        ast_vec_bytecode(prog, &args[2]);
                        prog.instructions.push(Instruction::SETVAR);
                    },
                    "if" => {
                        assert!(args.len() > 1);
                        *stackvals += 1;
                        let mut i = 1;
                        let mut end_jumps = Vec::new();
                        let mut prev_jump: usize;
                        while i < args.len() {
                            ast_vec_bytecode(prog, &args[i]);
                            if i == args.len() - 1 {
                                // else branch, break to avoid an unnecessary jump
                                break;
                            }
                            prev_jump = prog.instructions.len();
                            prog.instructions.push(Instruction::IFFALSE(0));
                            i += 1;
                            ast_vec_bytecode(prog, &args[i]);

                            let current_len = prog.instructions.len();
                            end_jumps.push(current_len);
                            prog.instructions.push(Instruction::GOTO(0));

                            // correct above cond jump to point past this branch
                            match &mut prog.instructions[prev_jump] {
                                Instruction::IFFALSE(p) => {
                                    // add one to skip the above jmp to end
                                    *p = current_len+1;
                                }
                                _ => unreachable!()
                            }
                            i += 1;
                        }
                        if args.len() % 2 == 1 {
                            // no else branch given, add a nil for a placeholder
                            prog.instructions.push(Instruction::PUSHNIL);
                        }
                        // correct end jumps to point past all the compiled branches
                        let current_len = prog.instructions.len();
                        for inst in end_jumps {
                            match &mut prog.instructions[inst] {
                                Instruction::GOTO(p) => {
                                    *p = current_len;
                                }
                                _ => unreachable!()
                            }
                        }
                    },
                    _ => {
                        panic!("unsupported function found");
                    },
                },
                _ => {
                    panic!("unsupported function found");
                },
            }
        },
        AST::Variable(args) => {
            //assert!(args.len() >= 1);
            ast_vec_bytecode(prog, &args[0]);
            prog.instructions.push(Instruction::DEREFVAR);
            *stackvals += 1;
        }
    }
}

fn ast_vec_bytecode(prog: &mut Program, astlist: &[AST]) {
    let mut stack_vals = 0;
    for ast in astlist {
        ast_bytecode(prog, ast, &mut stack_vals);
    }
    match stack_vals {
        0 => {
            // push dummy value
            prog.instructions.push(Instruction::PUSHNIL);
        },
        1 => {
            // single item remaining already
        },
        _ => {
            // concat values to a single item
            prog.instructions.push(Instruction::CONCAT(stack_vals));
        },
    }
}

pub fn generate_bytecode(ast: &[AST]) -> Program {
    let mut prog = Program {
        instructions: Vec::new()
    };

    ast_vec_bytecode(&mut prog, ast);
    prog.instructions.push(Instruction::END);

    return prog;
}