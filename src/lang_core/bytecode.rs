use crate::lang_core::parse::AST;

#[derive(Debug)]
pub enum Instruction {
    PUSH(String),
    CONCAT(usize),
    SETVAR,
    DEREFVAR,
    END,
}

pub struct Program {
    pub instructions: Vec<Instruction>
}

fn ast_bytecode(prog: &mut Program, ast: &AST) {
    match ast {
        AST::String(s) => {
            prog.instructions.push(Instruction::PUSH(s.to_owned()));
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
        }
    }
}

fn ast_vec_bytecode(prog: &mut Program, ast: &[AST]) {
    match ast {
        [] => {
            prog.instructions.push(Instruction::PUSH(String::from("")));
        },
        [a] => {
            ast_bytecode(prog, a);
        },
        _ => {
            for a in ast {
                ast_bytecode(prog, a);
            }
            prog.instructions.push(Instruction::CONCAT(ast.len() - 1));
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