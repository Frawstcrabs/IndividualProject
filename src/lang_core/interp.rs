use crate::bytecode::{Program, Instruction};
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Debug)]
pub enum VarValues {
    Nil,
    Str(String),
    Num(f64),
}

impl ToString for VarValues {
    fn to_string(&self) -> String {
        match self {
            VarValues::Nil => {
                String::new()
            },
            VarValues::Str(s) => {
                s.to_owned()
            },
            VarValues::Num(v) => {
                v.to_string()
            },
        }
    }
}

impl From<&VarValues> for bool {
    fn from(v: &VarValues) -> Self {
        match v {
            VarValues::Nil => false,
            VarValues::Str(s) => !s.is_empty() && s != "0",
            VarValues::Num(v) => *v != 0.0,
        }
    }
}

// todo: add actual garbage collector
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
        let mut inst = 0;
        loop {
            println!("stack: {:?}", self.stack);
            println!("instr: {}, {:?}", inst, prog.instructions[inst]);
            match &prog.instructions[inst] {
                Instruction::PUSHSTR(s) => {
                    self.stack.push(Gc::new(RefCell::new(VarValues::Str(s.to_owned()))));
                },
                Instruction::PUSHNIL => {
                    self.stack.push(Gc::new(RefCell::new(VarValues::Nil)));
                },
                Instruction::IFFALSE(i) => {
                    let test: bool = (&*self.stack.pop().unwrap().borrow()).into();
                    if !test {
                        inst = *i;
                        continue;
                    }
                },
                Instruction::GOTO(i) => {
                    inst = *i;
                    continue;
                },
                Instruction::CONCAT(n) => {
                    let n = *n;
                    if n >= 2 {
                        let mut values = self.stack.split_off(self.stack.len() - n)
                            .into_iter()
                            .filter(|v| {
                                match &*v.borrow() {
                                    VarValues::Nil => false,
                                    _ => true
                                }
                            })
                            .collect::<Vec<_>>();
                        match values.len() {
                            0 => {
                                // only nil values found
                                self.stack.push(Gc::new(RefCell::new(VarValues::Nil)));
                            }
                            1 => {
                                // nothing else to concat with
                                self.stack.push(values.pop().unwrap());
                            }
                            _ => {
                                // multiple items, need to convert to strings first
                                let strings = values.into_iter()
                                    .map(|v| v.borrow().to_string())
                                    .collect::<Vec<_>>();
                                let string_len = strings.iter().map(|s| s.len()).sum();
                                let mut new_string = String::with_capacity(string_len);
                                for s in strings {
                                    new_string.push_str(&s);
                                }
                                self.stack.push(Gc::new(RefCell::new(VarValues::Str(new_string))));
                            }
                        }
                    }
                },
                Instruction::SETVAR => {
                    let value = self.stack.pop().unwrap();
                    let name = self.stack.pop().unwrap().borrow().to_string();
                    self.vars.insert(name, value);
                },
                Instruction::DEREFVAR => {
                    let name = self.stack.pop().unwrap().borrow().to_string();
                    let var_value = match self.vars.get(&name) {
                        Some(v) => Gc::clone(v),
                        None => Gc::new(RefCell::new(VarValues::Str(format!("<{}:unknown var>", name))))
                    };
                    self.stack.push(var_value);
                },
                Instruction::END => {
                    break;
                },
            }
            inst += 1;
        }
    }
}