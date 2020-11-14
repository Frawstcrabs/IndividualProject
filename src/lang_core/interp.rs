use crate::bytecode::{Program, Instruction};
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Debug)]
pub enum VarValues {
    Str(String),
    Num(f64),
}

impl ToString for VarValues {
    fn to_string(&self) -> String {
        match self {
            VarValues::Str(s) => {
                s.to_owned()
            },
            VarValues::Num(v) => {
                v.to_string()
            }
        }
    }
}

type Gc<T> = Rc<T>;

pub struct Context {
    pub stack: Vec<Gc<RefCell<VarValues>>>,
    vars: HashMap<String, Gc<RefCell<VarValues>>>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            stack: Vec::new(),
            vars: HashMap::new(),
        }
    }
    pub fn interpret(&mut self, prog: &Program) {
        for instruction in &prog.instructions {
            match instruction {
                Instruction::PUSH(s) => {
                    self.stack.push(Rc::new(RefCell::new(VarValues::Str(s.to_owned()))));
                }
                Instruction::CONCAT(n) => {
                    let n = *n;
                    if n >= 2 {
                        let strings = self.stack.split_off(self.stack.len() - n).iter()
                            .map(|v| v.borrow().to_string())
                            .collect::<Vec<_>>();
                        let string_len = strings.iter().map(|s| s.len()).sum();
                        let mut new_string = String::with_capacity(string_len);
                        for s in strings {
                            new_string.push_str(&s);
                        }
                        self.stack.push(Rc::new(RefCell::new(VarValues::Str(new_string))));
                    }
                }
                Instruction::SETVAR => {
                    let value = self.stack.remove(self.stack.len() - 1);
                    let name = self.stack.remove(self.stack.len() - 1).borrow().to_string();
                    self.vars.insert(name, value.clone());
                }
                Instruction::DEREFVAR => {
                    let name = self.stack.remove(self.stack.len() - 1).borrow().to_string();
                    self.stack.push(self.vars.get(&name).unwrap().clone());
                }
                Instruction::END => {
                    break;
                }
            }
        }
    }
}