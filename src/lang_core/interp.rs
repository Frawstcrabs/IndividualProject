use crate::bytecode::Instruction;
use crate::builtins::register_builtins;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;

pub enum LangError {
    Throw(Gc<VarValues>),
}
pub(crate) type LangResult<T> = Result<T, LangError>;

pub enum VarValues {
    Nil,
    Str(String),
    Num(f64),
    AstStr(String, Option<f64>),
    Func(Vec<String>, Vec<Instruction>, Gc<Namespace>),
    RustFunc(fn(&mut Context, Vec<Gc<VarValues>>) -> LangResult<Gc<VarValues>>),
    RustClosure(Box<dyn Fn(&mut Context, Vec<Gc<VarValues>>) -> LangResult<Gc<VarValues>>>),
    List(Vec<Gc<VarValues>>),
}

pub fn f64_to_string(n: f64) -> String {
    let mut ret = n.to_string();
    if ret.ends_with(".0") {
        ret.truncate(ret.len() - 2);
    }
    ret
}

pub fn string_to_f64(s: &str) -> Option<f64> {
    if s.starts_with("0b") {
        u64::from_str_radix(s, 2).map(|v| v as f64).ok()
    } else if s.starts_with("0x") {
        u64::from_str_radix(s, 16).map(|v| v as f64).ok()
    } else {
        s.parse::<f64>().ok()
    }
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
            VarValues::AstStr(s, _) => {
                s.clone()
            },
            VarValues::Func(_, _, _) | VarValues::RustFunc(_) | VarValues::RustClosure(_) => {
                String::from("<Function>")
            },
            VarValues::List(_) => {
                String::from("<List>")
            }
        }
    }
}

impl From<&VarValues> for bool {
    fn from(v: &VarValues) -> Self {
        match v {
            VarValues::Nil => {
                false
            },
            VarValues::Str(s) |
            VarValues::AstStr(s, None) => {
                !s.is_empty() && s != "0"
            },
            VarValues::Num(v) |
            VarValues::AstStr(_, Some(v)) => {
                *v != 0.0
            },
            VarValues::Func(_, _, _) |
            VarValues::RustFunc(_) |
            VarValues::RustClosure(_) => {
                true
            },
            VarValues::List(vs) => {
                !vs.is_empty()
            },
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
            VarValues::AstStr(s, v) => {
                fmt.debug_tuple("AstStr")
                    .field(s)
                    .field(v)
                    .finish()
            },
            VarValues::Func(names, inst, _) => {
                fmt.debug_tuple("Func")
                    .field(names)
                    .field(inst)
                    .field(&format_args!("_"))
                    .finish()
            },
            VarValues::RustFunc(_) => {
                fmt.debug_tuple("RustFunc")
                    .field(&format_args!("_"))
                    .finish()
            },
            VarValues::RustClosure(_) => {
                fmt.debug_tuple("RustClosure")
                    .field(&format_args!("_"))
                    .finish()
            },
            VarValues::List(vs) => {
                fmt.debug_tuple("List")
                    .field(vs)
                    .finish()
            },
        }
    }
}

#[macro_export]
macro_rules! throw_string {
    ($($args:expr),+) => {
        Err(LangError::Throw(
            Gc::new(RefCell::new(
                VarValues::Str(format!($($args),+))
            ))
        ))
    };
}

impl VarValues {
    fn call(&self, ctx: &mut Context, args: Vec<Gc<VarValues>>) -> LangResult<()> {
        match self {
            VarValues::Func(names, inst, outer_scope) => {
                let mut vars = HashMap::with_capacity(args.len());
                if names.len() > args.len() {
                    return throw_string!("expected {} args, got {}", names.len(), args.len());
                }
                for i in 0..names.len() {
                    vars.insert(names[i].clone(), VarRefType::Value(Gc::clone(&args[i])));
                }
                if names.iter().all(|v| v != "args") {
                    vars.insert(
                        String::from("args"),
                        VarRefType::Value(
                            Gc::new(RefCell::new(
                                VarValues::List(args)
                            ))
                        )
                    );
                }
                let old_scope = Gc::clone(&ctx.cur_scope);
                let new_ns = Gc::new(RefCell::new(Namespace {
                    vars,
                    outer_scope: Some(Gc::clone(&outer_scope)),
                }));
                ctx.cur_scope = new_ns;
                ctx.interpret(inst)?;
                ctx.cur_scope = old_scope;
                Ok(())
            },
            VarValues::RustFunc(f) => {
                let ret_val = f(ctx, args)?;
                ctx.stack.push(ret_val);
                Ok(())
            },
            VarValues::RustClosure(f) => {
                let ret_val = f(ctx, args)?;
                ctx.stack.push(ret_val);
                Ok(())
            },
            _ => {
                throw_string!("uncallable object")
            },
        }
    }
    fn get_attr(&self, obj: Gc<VarValues>, index: Gc<VarValues>) -> LangResult<Gc<VarValues>> {
        match self {
            VarValues::List(_) => {
                let name = index.borrow().to_string();
                match &name[..] {
                    "push" => {
                        let method = move |_ctx: &mut Context, args: Vec<Gc<VarValues>>| {
                            match &mut *obj.borrow_mut() {
                                VarValues::List(vals) => {
                                    vals.extend(args);
                                    Ok(Gc::new(RefCell::new(VarValues::Nil)))
                                }
                                _ => unreachable!()
                            }
                        };
                        Ok(
                            Gc::new(RefCell::new(
                                VarValues::RustClosure(Box::new(method))
                            ))
                        )
                    },
                    _ => {
                        throw_string!("invalid attr")
                    }
                }
            },
            _ => {
                throw_string!("cannot get attr")
            },
        }
    }

    fn get_index(&self, obj: Gc<VarValues>, index: Gc<VarValues>) -> LangResult<Gc<VarValues>> {
        match self {
            VarValues::List(vs) => {
                match &*index.borrow() {
                    VarValues::Str(s) |
                    VarValues::AstStr(s, None) => {
                        let v = s.parse::<isize>().unwrap();
                        if v < 0 || v as usize >= vs.len() {
                            return throw_string!("index out of range");
                        }
                        Ok(Gc::clone(&vs[v as usize]))
                    },
                    VarValues::Num(n) |
                    VarValues::AstStr(_, Some(n))=> {
                        let v = *n as usize;
                        Ok(Gc::clone(&vs[v]))
                    },
                    _ => {
                        throw_string!("invalid index")
                    },
                }
            },
            _ => {
                throw_string!("cannot index")
            }
        }
    }

    fn set_index(&mut self, obj: Gc<VarValues>, index: Gc<VarValues>, val: Gc<VarValues>) -> LangResult<()> {
        throw_string!("cannot set index")
    }

    fn set_attr(&mut self, obj: Gc<VarValues>, index: Gc<VarValues>, val: Gc<VarValues>) -> LangResult<()> {
        throw_string!("cannot set attr")
    }
}

// todo: add actual garbage collector
pub type Gc<T> = Rc<RefCell<T>>;

#[derive(Debug)]
pub enum VarRefType {
    Value(Gc<VarValues>),
    NonLocal,
}

pub struct Namespace {
    vars: HashMap<String, VarRefType>,
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
    fn interpret_inst(&mut self, prog: &[Instruction], counter: &mut usize) -> LangResult<()> {
        match &prog[*counter] {
            Instruction::PUSHSTR(s) => {
                self.stack.push(
                    Gc::new(RefCell::new(
                        VarValues::Str(s.clone())
                    ))
                );
            },
            Instruction::PUSHASTSTR(s, v) => {
                self.stack.push(
                    Gc::new(RefCell::new(
                        VarValues::AstStr(s.clone(), *v)
                    ))
                );
            },
            Instruction::PUSHNIL => {
                self.stack.push(Gc::new(RefCell::new(VarValues::Nil)));
            },
            Instruction::IFFALSE(i) => {
                let test: bool = (&*self.stack.pop().unwrap().borrow()).into();
                if !test {
                    *counter = *i;
                    return Ok(());
                }
            },
            Instruction::GOTO(i) => {
                *counter = *i;
                return Ok(());
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
                let mut ns = Gc::clone(&self.cur_scope);
                loop {
                    let cur_ns = Gc::clone(&ns);
                    let mut ns_ref = cur_ns.borrow_mut();
                    match ns_ref.vars.get_mut(&name) {
                        Some(VarRefType::NonLocal) => match &ns_ref.outer_scope {
                            Some(new_ns) => {
                                ns = Gc::clone(new_ns);
                            }
                            None => {
                                panic!("no variable reference found");
                            }
                        }
                        Some(VarRefType::Value(v)) => {
                            *v = value;
                            break;
                        }
                        None => {
                            ns_ref.vars.insert(name, VarRefType::Value(value));
                            break;
                        }
                    }
                }
            },
            Instruction::SETATTR => {
                let val = self.stack.pop().unwrap();
                let index = self.stack.pop().unwrap();
                let obj = self.stack.pop().unwrap();
                let obj_clone = Gc::clone(&obj);
                obj.borrow_mut().set_attr(obj_clone, index, val)?;
            },
            Instruction::SETINDEX => {
                let val = self.stack.pop().unwrap();
                let index = self.stack.pop().unwrap();
                let obj = self.stack.pop().unwrap();
                let obj_clone = Gc::clone(&obj);
                obj.borrow_mut().set_index(obj_clone, index, val)?;
            },
            Instruction::SETNONLOCAL => {
                let name = self.stack.pop().unwrap().borrow().to_string();
                self.cur_scope.borrow_mut().vars.insert(name, VarRefType::NonLocal);
            },
            Instruction::GETVAR => {
                let name = self.stack.pop().unwrap().borrow().to_string();
                let mut ns = Gc::clone(&self.cur_scope);
                let var_value;
                loop {
                    let cur_ns = Gc::clone(&ns);
                    let ns_ref = cur_ns.borrow();
                    match ns_ref.vars.get(&name) {
                        Some(VarRefType::Value(v)) => {
                            var_value = Gc::clone(v);
                            break;
                        }
                        Some(VarRefType::NonLocal) | None => match &ns_ref.outer_scope {
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
            Instruction::GETATTR => {
                let index = self.stack.pop().unwrap();
                let obj = self.stack.pop().unwrap();
                let obj_clone = Gc::clone(&obj);
                self.stack.push(obj.borrow().get_attr(obj_clone, index)?);
            },
            Instruction::GETINDEX => {
                let index = self.stack.pop().unwrap();
                let obj = self.stack.pop().unwrap();
                let obj_clone = Gc::clone(&obj);
                self.stack.push(obj.borrow().get_index(obj_clone, index)?);
            },
            Instruction::DELVAR => {

            },
            Instruction::DELATTR => {

            },
            Instruction::DELINDEX => {

            },
            Instruction::CREATEFUNC(arg_names, loc, size) => {
                let loc = *loc;
                let size = *size;
                self.stack.push(
                    Gc::new(RefCell::new(VarValues::Func(
                        arg_names.clone(),
                        prog[loc..loc+size].to_vec(),
                        Gc::clone(&self.cur_scope)
                    )))
                );
            },
            Instruction::CALLFUNC(arg_size) => {
                let arg_size = *arg_size;
                let args = self.stack.split_off(self.stack.len() - arg_size);
                let called_var = self.stack.pop().unwrap();
                called_var.borrow().call(self, args)?;
            },
            Instruction::CREATELIST(n) => {
                let vals = self.stack.split_off(self.stack.len() - n);
                self.stack.push(
                    Gc::new(RefCell::new(
                        VarValues::List(vals)
                    ))
                );
            },
            Instruction::STARTCATCH(loc) => {
                let stack_size = self.stack.len();
                *counter += 1;
                match self.catch_block(prog, counter) {
                    Ok(_) => {
                        // TODO: wrap catch value in status object
                    },
                    Err(LangError::Throw(v)) => {
                        self.stack.truncate(stack_size);
                        // TODO: wrap catch value in status object
                        self.stack.push(v);
                        *counter = *loc;
                        return Ok(());
                    }
                }
            },
            Instruction::THROWVAL => {
                let v = self.stack.pop().unwrap();
                return Err(LangError::Throw(v));
            },
            Instruction::END | Instruction::ENDCATCH => unimplemented!(),
        }
        *counter += 1;
        Ok(())
    }
    fn catch_block(&mut self, prog: &[Instruction], counter: &mut usize) -> LangResult<()> {
        loop {
            println!("stack: {:?}", self.stack);
            println!("instr: {}, {:?}", *counter, prog[*counter]);
            match &prog[*counter] {
                Instruction::ENDCATCH => {
                    break;
                },
                Instruction::END => {
                    panic!("found end inside of catch block")
                }
                _ => self.interpret_inst(prog, counter)?
            }
        }
        Ok(())
    }
    pub fn interpret(&mut self, prog: &[Instruction]) -> LangResult<()> {
        let mut counter = 0;
        loop {
            println!("stack: {:?}", self.stack);
            println!("instr: {}, {:?}", counter, prog[counter]);
            match &prog[counter] {
                Instruction::END => {
                    break;
                },
                Instruction::ENDCATCH => {
                    panic!("found endcatch outside of catch block");
                },
                _ => self.interpret_inst(prog, &mut counter)?
            }
        }
        Ok(())
    }
}
