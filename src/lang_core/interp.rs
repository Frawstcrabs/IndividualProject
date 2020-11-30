use crate::bytecode::Instruction;
use crate::builtins::register_builtins;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;

pub enum VarValues {
    Nil,
    Str(String),
    Num(f64),
    Func(Vec<Instruction>, Gc<Namespace>),
    RustFunc(fn(&mut Context, Vec<Gc<VarValues>>) -> Gc<VarValues>),
}

pub fn f64_to_string(n: f64) -> String {
    let mut ret = n.to_string();
    if ret.ends_with(".0") {
        ret.truncate(ret.len() - 2);
    }
    ret
}

impl ToString for VarValues {
    fn to_string(&self) -> String {
        match self {
            VarValues::Nil => {
                String::new()
            },
            VarValues::Str(s) => {
                s.clone()
            },
            VarValues::Num(v) => {
                f64_to_string(*v)
            },
            VarValues::Func(_, _) | VarValues::RustFunc(_) => {
                String::from("<Function>")
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
            VarValues::Func(_, _) | VarValues::RustFunc(_) => true,
        }
    }
}

impl fmt::Debug for VarValues {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VarValues::Nil => {
                fmt.write_fmt(format_args!("Nil"))
            },
            VarValues::Str(s) => {
                fmt.debug_tuple("Str")
                    .field(s)
                    .finish()
            },
            VarValues::Num(n) => {
                fmt.debug_tuple("Num")
                    .field(n)
                    .finish()
            },
            VarValues::Func(inst, _) => {
                fmt.debug_tuple("Func")
                    .field(inst)
                    .field(&format_args!("_"))
                    .finish()
            },
            VarValues::RustFunc(_) => {
                fmt.debug_tuple("RustFunc")
                    .field(&format_args!("_"))
                    .finish()
            }
        }
    }
}

impl VarValues {
    fn call(&self, ctx: &mut Context, args: Vec<Gc<VarValues>>) {
        match self {
            VarValues::Func(inst, outer_scope) => {
                let mut vars = HashMap::with_capacity(args.len());
                match &inst[0] {
                    Instruction::FUNCHEADER(names) => {
                        assert!(names.len() <= args.len());
                        for i in 0..names.len() {
                            vars.insert(names[i].clone(), Gc::clone(&args[i]));
                        }
                    },
                    _ => unreachable!(),
                }
                let old_scope = Gc::clone(&ctx.cur_scope);
                let new_ns = Gc::new(RefCell::new(Namespace {
                    vars,
                    outer_scope: Some(Gc::clone(&outer_scope)),
                }));
                ctx.cur_scope = new_ns;
                ctx.interpret(inst);
                ctx.cur_scope = old_scope;
            },
            VarValues::RustFunc(f) => {
                let ret_val = f(ctx, args);
                ctx.stack.push(ret_val);
            }
            _ => panic!("tried to call non-function type"),
        }
    }
}

// todo: add actual garbage collector
pub type Gc<T> = Rc<RefCell<T>>;

pub struct Namespace {
    vars: HashMap<String, Gc<VarValues>>,
    outer_scope: Option<Gc<Namespace>>,
}

impl fmt::Debug for Namespace {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Namespace")
            .field("vars", &self.vars)
            .field("outer_scope", &format_args!("_"))
            .finish()
    }
}

pub struct Context {
    pub stack: Vec<Gc<VarValues>>,
    cur_scope: Gc<Namespace>,
    global_scope: Gc<Namespace>,
}

impl Context {
    pub fn new() -> Self {
        let mut global_vars = HashMap::new();
        register_builtins(&mut global_vars);
        let global_scope = Gc::new(RefCell::new(Namespace {
            vars: global_vars,
            outer_scope: None,
        }));
        Context {
            stack: Vec::new(),
            cur_scope: Gc::clone(&global_scope),
            global_scope,
        }
    }
    pub fn interpret(&mut self, prog: &[Instruction]) {
        let mut inst = 0;
        loop {
            println!("stack: {:?}", self.stack);
            println!("instr: {}, {:?}", inst, prog[inst]);
            match &prog[inst] {
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
                    self.cur_scope.borrow_mut().vars.insert(name, value);
                },
                Instruction::DEREFVAR => {
                    let name = self.stack.pop().unwrap().borrow().to_string();
                    let mut ns = Gc::clone(&self.cur_scope);
                    let var_value;
                    loop {
                        let cur_ns = Gc::clone(&ns);
                        let ns_ref = cur_ns.borrow();
                        match ns_ref.vars.get(&name) {
                            Some(v) => {
                                var_value = Gc::clone(v);
                                break;
                            }
                            None => match &ns_ref.outer_scope {
                                Some(new_ns) => {
                                    ns = Gc::clone(new_ns);
                                }
                                None => {
                                    var_value = Gc::new(RefCell::new(VarValues::Str(format!("<{}:unknown var>", name))));
                                    break;
                                }
                            }
                        }
                    }
                    self.stack.push(var_value);
                },
                Instruction::FUNCHEADER(_) => {
                    // this is just for holding data
                    // is a no-op
                }
                Instruction::CREATEFUNC(loc, size) => {
                    let loc = *loc;
                    let size = *size;
                    self.stack.push(
                        Gc::new(RefCell::new(VarValues::Func(
                            prog[loc..loc+size].to_vec(),
                            Gc::clone(&self.cur_scope)
                        )))
                    );
                }
                Instruction::CALLFUNC(arg_size) => {
                    let arg_size = *arg_size;
                    let args = self.stack.split_off(self.stack.len() - arg_size);
                    let called_var = self.stack.pop().unwrap();
                    called_var.borrow_mut().call(self, args);
                }
                Instruction::END => {
                    break;
                },
            }
            inst += 1;
        }
    }
}