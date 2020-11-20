use crate::lang_core::parse::AST;

#[derive(Debug)]
pub enum Instruction {
    PUSH(String),
    PUSHNIL,
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
            prog.instructions.push(Instruction::PUSH(s.to_owned()));
            *stackvals += 1;
        }
        AST::Function(args) => {
            let name = &args[0];
            match &name[..] {
                [AST::String(s)] => match &s[..] {
                    "set" => {
                        // set command read
                        assert_eq!(args.len(), 3);
                        ast_vec_bytecode(prog, &args[1]);
                        ast_vec_bytecode(prog, &args[2]);
                        prog.instructions.push(Instruction::SETVAR);
                    }
                    _ => {
                        panic!("unsupported function found");
                    }
                }
                _ => {
                    panic!("unsupported function found");
                }
            }
        }
        AST::Variable(args) => {
            assert!(args.len() >= 1);
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
        }
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