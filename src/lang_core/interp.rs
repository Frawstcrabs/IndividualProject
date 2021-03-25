use crate::bytecode::Instruction;
use crate::builtins::register_builtins;
use crate::builtins::math::val_to_f64;
use crate::builtins::boolean::test_equality;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use libgc::{Gc as Gc_};
use std::ops::{Deref, DerefMut};

pub enum LangError {
    Throw(Gc<VarValues>),
    CatchUnwind(usize),
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
    CatchResult(bool, Gc<VarValues>),
    List(Vec<Gc<VarValues>>),
    Map(HashMap<String, Gc<VarValues>>),
}

// SAFETY: libgc needs these traits but the lib
// only supports single-threaded applications,
// so there won't be any concurrency issues
unsafe impl Send for VarValues {}
unsafe impl Sync for VarValues {}

// wrapper type so that RefCell can be used within
// Gc types
// as of right now, libgc is only single-threaded
// anyway, and the interpreter only allows single-
// threading, so there won't be concurrency issues
#[repr(transparent)]
pub struct SendSyncRefCell<T>(pub RefCell<T>);

unsafe impl<T> Send for SendSyncRefCell<T> {}
unsafe impl<T> Sync for SendSyncRefCell<T> {}

impl<T> Deref for SendSyncRefCell<T> {
    type Target = RefCell<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for SendSyncRefCell<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: fmt::Debug> fmt::Debug for SendSyncRefCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
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
        u64::from_str_radix(&s[2..], 2).map(|v| v as f64).ok()
    } else if s.starts_with("0x") {
        u64::from_str_radix(&s[2..], 16).map(|v| v as f64).ok()
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
            VarValues::Func(_, _, _) |
            VarValues::RustFunc(_) |
            VarValues::RustClosure(_) => {
                String::from("<Function>")
            },
            VarValues::CatchResult(_, v) => {
                v.borrow().to_string()
            },
            VarValues::List(_) => {
                String::from("<List>")
            },
            VarValues::Map(_) => {
                String::from("<Map>")
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
            VarValues::CatchResult(is_success, _) => {
                *is_success
            },
            VarValues::List(vs) => {
                !vs.is_empty()
            },
            VarValues::Map(vs) => {
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
            VarValues::CatchResult(is_success, v) => {
                fmt.debug_tuple("CatchResult")
                    .field(is_success)
                    .field(v)
                    .finish()
            },
            VarValues::List(vs) => {
                fmt.debug_tuple("List")
                    .field(vs)
                    .finish()
            },
            VarValues::Map(vs) => {
                fmt.debug_tuple("Map")
                    .field(vs)
                    .finish()
            },
        }
    }
}

pub fn new_value<T>(val: T) -> Gc<T> {
    Gc::new(SendSyncRefCell(RefCell::new(val)))
}

#[macro_export]
macro_rules! throw_string {
    ($($args:expr),+) => {
        Err(LangError::Throw(
            new_value(
                VarValues::Str(format!($($args),+))
            )
        ))
    };
}

fn validate_list_index(mut v: f64, max: usize) -> LangResult<usize> {
    if v.fract() != 0.0 {
        return throw_string!("invalid index");
    }
    if v < 0.0 {
        v += max as f64;
    }
    if v < 0.0 || v as usize >= max {
        return throw_string!("index out of range");
    }
    Ok(v as usize)
}

fn index_val_str(s: &str, index: f64) -> LangResult<Gc<VarValues>> {
    if index.fract() != 0.0 {
        return throw_string!("invalid index");
    }
    let index = index as isize;
    if index >= 0 {
        let index = index as usize;
        match s.chars().nth(index) {
            Some(c) => Ok(new_value(VarValues::Str(c.to_string()))),
            None => throw_string!("index out of range")
        }
    } else {
        let index = (-1 - index) as usize;
        match s.chars().nth_back(index) {
            Some(c) => Ok(new_value(VarValues::Str(c.to_string()))),
            None => throw_string!("index out of range")
        }
    }
}

impl VarValues {
    fn call(&self, ctx: &mut Context, args: Vec<Gc<VarValues>>, outputter: &mut dyn Outputter) -> LangResult<()> {
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
                            new_value(
                                VarValues::List(args)
                            )
                        )
                    );
                }
                let old_scope = Gc::clone(&ctx.cur_scope);
                let new_ns = new_value(Namespace {
                    vars,
                    outer_scope: Some(Gc::clone(&outer_scope)),
                });
                ctx.cur_scope = new_ns;
                ctx.interpret(inst, outputter)?;
                ctx.cur_scope = old_scope;
                Ok(())
            },
            VarValues::RustFunc(f) => {
                let ret_val = f(ctx, args)?;
                outputter.output_value(ret_val);
                Ok(())
            },
            VarValues::RustClosure(f) => {
                let ret_val = f(ctx, args)?;
                outputter.output_value(ret_val);
                Ok(())
            },
            _ => {
                throw_string!("uncallable object")
            },
        }
    }

    fn get_attr(&self, obj: Gc<VarValues>, index: Gc<VarValues>) -> LangResult<Gc<VarValues>> {
        match self {
            VarValues::List(vs) => {
                let name = index.borrow().to_string();
                match &name[..] {
                    "push" => {
                        let method = move |_ctx: &mut Context, args: Vec<Gc<VarValues>>| {
                            match &mut *obj.borrow_mut() {
                                VarValues::List(vals) => {
                                    vals.extend(args);
                                    Ok(new_value(VarValues::Nil))
                                }
                                _ => unreachable!()
                            }
                        };
                        Ok(
                            new_value(
                                VarValues::RustClosure(Box::new(method))
                            )
                        )
                    },
                    "index" => {
                        let method = move |_ctx: &mut Context, args: Vec<Gc<VarValues>>| {
                            if args.len() != 1 {
                                return throw_string!("<list.index:expected 1 arg, got {}", args.len());
                            }
                            let arg = &args[0];
                            match &mut *obj.borrow_mut() {
                                VarValues::List(vals) => {
                                    for i in 0..vals.len() {
                                        if test_equality(&vals[i], arg) {
                                            return Ok(new_value(VarValues::Num(i as f64)));
                                        }
                                    }
                                    Ok(new_value(VarValues::Num(-1.0)))
                                }
                                _ => unreachable!()
                            }
                        };
                        Ok(
                            new_value(
                                VarValues::RustClosure(Box::new(method))
                            )
                        )
                    },
                    "length" => {
                        Ok(new_value(VarValues::Num(vs.len() as f64)))
                    },
                    _ => {
                        throw_string!("invalid attr")
                    }
                }
            },
            VarValues::Map(vals) => {
                let name = index.borrow().to_string();
                match &name[..] {
                    "length" => {
                        Ok(new_value(VarValues::Num(vals.len() as f64)))
                    },
                    "keys" => {
                        Ok(new_value(
                            VarValues::List(
                                vals.keys()
                                .map(|v| {
                                    new_value(VarValues::Str(v.to_owned()))
                                })
                                .collect()
                            )
                        ))
                    },
                    "values" => {
                        Ok(new_value(
                            VarValues::List(
                                vals.values().map(|v| *v).collect()
                            )
                        ))
                    }
                    _ => {
                        throw_string!("invalid attr")
                    }
                }
            },
            VarValues::Str(s) |
            VarValues::AstStr(s, _) => {
                let name = index.borrow().to_string();
                match &name[..] {
                    "length" => {
                        Ok(new_value(VarValues::Num(s.chars().count() as f64)))
                    },
                    _ => {
                        throw_string!("invalid attr")
                    }
                }
            },
            VarValues::Num(n) => {
                let name = index.borrow().to_string();
                match &name[..] {
                    "length" => {
                        Ok(new_value(VarValues::Num(f64_to_string(*n).chars().count() as f64)))
                    },
                    _ => {
                        throw_string!("invalid attr")
                    }
                }
            },
            VarValues::Nil => {
                let name = index.borrow().to_string();
                match &name[..] {
                    "length" => {
                        Ok(new_value(VarValues::Num(0.0)))
                    },
                    _ => {
                        throw_string!("invalid attr")
                    }
                }
            },
            VarValues::CatchResult(is_success, v) => {
                let name = index.borrow().to_string();
                match &name[..] {
                    "status" => {
                        Ok(
                            new_value(
                                VarValues::Num(if *is_success {1.0} else {0.0})
                            )
                        )
                    },
                    "value" => {
                        Ok(Gc::clone(v))
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

    fn get_index(&self, _obj: Gc<VarValues>, index: Gc<VarValues>) -> LangResult<Gc<VarValues>> {
        match self {
            VarValues::List(vs) => {
                let i = match &*index.borrow() {
                    VarValues::Str(s) => {
                        match string_to_f64(s) {
                            Some(v) => validate_list_index(v, vs.len())?,
                            None => {
                                return throw_string!("invalid index");
                            }
                        }
                    },
                    VarValues::Num(n) |
                    VarValues::AstStr(_, Some(n))=> {
                        validate_list_index(*n, vs.len())?
                    },
                    _ => {
                        return throw_string!("invalid index");
                    },
                };
                Ok(Gc::clone(&vs[i]))
            },
            VarValues::Map(vals) => {
                let index = index.borrow().to_string();
                match vals.get(&index) {
                    Some(v) => Ok(*v),
                    None => return throw_string!("<map:{}:unknown key>", index),
                }
            },
            VarValues::Str(s) |
            VarValues::AstStr(s, _) => {
                let v = match &*index.borrow() {
                    VarValues::Str(s) => {
                        match string_to_f64(s) {
                            Some(v) => v,
                            None => {
                                return throw_string!("invalid index");
                            }
                        }
                    },
                    VarValues::Num(n) |
                    VarValues::AstStr(_, Some(n))=> {
                        *n
                    },
                    _ => {
                        return throw_string!("invalid index");
                    },
                };
                index_val_str(s, v)
            },
            VarValues::Num(n) => {
                let v = match &*index.borrow() {
                    VarValues::Str(s) => {
                        match string_to_f64(s) {
                            Some(v) => v,
                            None => {
                                return throw_string!("invalid index");
                            }
                        }
                    },
                    VarValues::Num(n) |
                    VarValues::AstStr(_, Some(n))=> {
                        *n
                    },
                    _ => {
                        return throw_string!("invalid index");
                    },
                };
                index_val_str(&f64_to_string(*n), v)
            }
            _ => {
                throw_string!("cannot index")
            },
        }
    }

    fn set_index(&mut self, _obj: Gc<VarValues>, index: Gc<VarValues>, val: Gc<VarValues>) -> LangResult<()> {
        match self {
            VarValues::List(vs) => {
                let v = match &*index.borrow() {
                    VarValues::Str(s) => {
                        match string_to_f64(s) {
                            Some(v) => validate_list_index(v, vs.len())?,
                            None => {
                                return throw_string!("invalid index");
                            }
                        }
                    },
                    VarValues::Num(n) |
                    VarValues::AstStr(_, Some(n))=> {
                        validate_list_index(*n, vs.len())?
                    },
                    _ => {
                        return throw_string!("invalid index");
                    },
                };
                vs[v] = val;
                Ok(())
            },
            VarValues::Map(vals) => {
                let index = index.borrow().to_string();
                vals.insert(index, val);
                Ok(())
            },
            _ => {
                throw_string!("cannot set index")
            },
        }
    }

    fn set_attr(&mut self, _obj: Gc<VarValues>, _index: Gc<VarValues>, _val: Gc<VarValues>) -> LangResult<()> {
        throw_string!("cannot set attr")
    }

    fn del_index(&mut self, index: Gc<VarValues>) -> LangResult<()> {
        match self {
            VarValues::List(vs) => {
                let v = match &*index.borrow() {
                    VarValues::Str(s) => {
                        match string_to_f64(s) {
                            Some(v) => validate_list_index(v, vs.len())?,
                            None => {
                                return throw_string!("invalid index");
                            }
                        }
                    },
                    VarValues::Num(n) |
                    VarValues::AstStr(_, Some(n))=> {
                        validate_list_index(*n, vs.len())?
                    },
                    _ => {
                        return throw_string!("invalid index");
                    },
                };
                vs.remove(v);
                Ok(())
            }
            VarValues::Map(vals) => {
                let index = index.borrow().to_string();
                vals.remove(&index);
                Ok(())
            },
            _ => {
                throw_string!("cannot del index")
            }
        }
    }

    fn del_attr(&mut self, _index: Gc<VarValues>) -> LangResult<()> {
        throw_string!("cannot del attr")
    }
}

pub type Gc<T> = Gc_<SendSyncRefCell<T>>;

#[derive(Debug)]
pub enum VarRefType {
    Value(Gc<VarValues>),
    NonLocal,
}

pub struct Namespace {
    vars: HashMap<String, VarRefType>,
    outer_scope: Option<Gc<Namespace>>,
}

// TODO: check if this is safe
unsafe impl Send for Namespace {}
unsafe impl Sync for Namespace {}

impl fmt::Debug for Namespace {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Namespace")
            .field("vars", &self.vars)
            .field("outer_scope", &format_args!("_"))
            .finish()
    }
}

struct LoopFrame {
    stack_vals: usize,
    loop_data: LoopType,
}

enum LoopType {
    While,
    For {
        ident: String,
        value: f64,
        step: f64,
        end: f64,
    },
}

pub trait Outputter {
    fn output_string(&mut self, s: &str, v: Option<f64>);
    fn output_value(&mut self, v: Gc<VarValues>);
}

pub struct StdOutOutput {}

impl Outputter for StdOutOutput {
    fn output_string(&mut self, s: &str, _: Option<f64>) {
        print!("{}", s);
    }

    fn output_value(&mut self, v: Gc<VarValues>) {
        print!("{}", v.borrow().to_string());
    }
}

pub struct CollectOutput {
    results: Vec<Gc<VarValues>>
}

impl Outputter for CollectOutput {
    fn output_string(&mut self, s: &str, v: Option<f64>) {
        self.results.push(new_value(VarValues::AstStr(s.to_owned(), v)));
    }

    fn output_value(&mut self, v: Gc<VarValues>) {
        self.results.push(Gc::clone(&v));
    }
}

pub struct Context {
    pub stack: Vec<Gc<VarValues>>,
    loop_stack: Vec<LoopFrame>,
    cur_scope: Gc<Namespace>,
}

fn concat_vals(values: Vec<Gc<VarValues>>) -> Gc<VarValues> {
    let mut values = values
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
            new_value(VarValues::Nil)
        }
        1 => {
            // nothing else to concat with
            values.pop().unwrap()
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
            new_value(VarValues::Str(new_string))
        }
    }
}

fn set_scope_var(name: String, value: Gc<VarValues>, mut ns: Gc<Namespace>) {
    loop {
        let cur_ns = Gc::clone(&ns);
        let mut ns_ref = cur_ns.borrow_mut();
        match ns_ref.vars.get_mut(&name) {
            Some(VarRefType::NonLocal) => match &ns_ref.outer_scope {
                Some(new_ns) => {
                    ns = Gc::clone(new_ns);
                }
                None => {
                    panic!("chain of nonlocals reached global scope");
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
}

impl Context {
    pub fn new() -> Self {
        let mut global_vars = HashMap::new();
        register_builtins(&mut global_vars);
        let global_scope = new_value(Namespace {
            vars: global_vars,
            outer_scope: None,
        });
        Context {
            stack: Vec::new(),
            loop_stack: Vec::new(),
            cur_scope: global_scope
        }
    }
    pub fn with_args(args: Vec<String>) -> Self {
        let mut global_vars = HashMap::new();
        register_builtins(&mut global_vars);
        let args_var = new_value(VarValues::List(
            args.into_iter()
                .map(|s| new_value(VarValues::Str(s)))
                .collect()
        ));
        global_vars.insert(String::from("args"), VarRefType::Value(args_var));
        let global_scope = new_value(Namespace {
            vars: global_vars,
            outer_scope: None,
        });
        Context {
            stack: Vec::new(),
            loop_stack: Vec::new(),
            cur_scope: global_scope
        }
    }
    #[inline]
    fn interpret_inst(&mut self, prog: &[Instruction], counter: &mut usize, outputter: &mut dyn Outputter) -> LangResult<()> {
        match &prog[*counter] {
            Instruction::PUSHSTR(s) => {
                self.stack.push(
                    new_value(
                        VarValues::Str(s.clone())
                    )
                );
            },
            Instruction::PUSHASTSTR(s, v) => {
                self.stack.push(
                    new_value(
                        VarValues::AstStr(s.clone(), *v)
                    )
                );
            },
            Instruction::PUSHNIL => {
                self.stack.push(new_value(VarValues::Nil));
            },
            Instruction::PUSHNUM(n) => {
                self.stack.push(new_value(VarValues::Num(*n)));
            }
            Instruction::OUTPUTSTR(s, v) => {
                outputter.output_string(s, *v);
            }
            Instruction::OUTPUTVAL => {
                let val = self.stack.pop().unwrap();
                outputter.output_value(val);
            }
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
                    let concat_val = concat_vals(self.stack.split_off(self.stack.len() - n));
                    self.stack.push(concat_val);
                }
            },
            Instruction::DROP(n) => {
                self.stack.truncate(self.stack.len() - *n);
            },
            Instruction::SETVAR(name) => {
                let value = self.stack.pop().unwrap();
                set_scope_var(name.clone(), value, Gc::clone(&self.cur_scope));
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
            Instruction::SETNONLOCAL(name) => {
                self.cur_scope.borrow_mut().vars.insert(name.clone(), VarRefType::NonLocal);
            },
            Instruction::GETVAR(name) => {
                let mut ns = Gc::clone(&self.cur_scope);
                let var_value;
                loop {
                    let cur_ns = Gc::clone(&ns);
                    let ns_ref = cur_ns.borrow();
                    match ns_ref.vars.get(name) {
                        Some(VarRefType::Value(v)) => {
                            var_value = Gc::clone(v);
                            break;
                        }
                        Some(VarRefType::NonLocal) | None => match &ns_ref.outer_scope {
                            Some(new_ns) => {
                                ns = Gc::clone(new_ns);
                            }
                            None => {
                                //println!("not found");
                                return throw_string!("<{}:unknown var>", name);
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
            Instruction::DELVAR(name) => {
                self.cur_scope.borrow_mut().vars.remove(name);
            },
            Instruction::DELATTR => {
                let index = self.stack.pop().unwrap();
                let obj = self.stack.pop().unwrap();
                obj.borrow_mut().del_attr(index)?;
            },
            Instruction::DELINDEX => {
                let index = self.stack.pop().unwrap();
                let obj = self.stack.pop().unwrap();
                obj.borrow_mut().del_index(index)?;
            },
            Instruction::CREATEFUNC(arg_names, loc, size) => {
                let loc = *loc;
                let size = *size;
                self.stack.push(
                    new_value(VarValues::Func(
                        arg_names.clone(),
                        prog[loc..loc+size].to_vec(),
                        Gc::clone(&self.cur_scope)
                    ))
                );
            },
            Instruction::CALLFUNC(arg_size, direct_output) => {
                let arg_size = *arg_size;
                let args = self.stack.split_off(self.stack.len() - arg_size);
                let called_var = self.stack.pop().unwrap();
                if *direct_output {
                    called_var.borrow().call(self, args, outputter)?;
                } else {
                    let mut collector = CollectOutput {
                        results: Vec::new(),
                    };
                    called_var.borrow().call(self, args, &mut collector)?;
                    self.stack.push(concat_vals(collector.results));
                }
            },
            Instruction::CREATELIST(n) => {
                let vals = self.stack.split_off(self.stack.len() - n);
                self.stack.push(
                    new_value(
                        VarValues::List(vals)
                    )
                );
            },
            Instruction::CREATEMAP(n) => {
                let vals = self.stack.split_off(self.stack.len() - n);
                let mut map = HashMap::with_capacity(n/2);
                let mut iter = vals.into_iter();
                for _ in 0..n/2 {
                    let key = iter.next().unwrap().borrow().to_string();
                    let value = iter.next().unwrap();
                    map.insert(key, value);
                }
                self.stack.push(
                    new_value(
                        VarValues::Map(map)
                    )
                );
            },
            Instruction::WHILESTART => {
                self.loop_stack.push(LoopFrame {
                    stack_vals: 0,
                    loop_data: LoopType::While,
                });
            },
            Instruction::FORSTART => {
                let step = val_to_f64(&self.stack.pop().unwrap(), "for")?;
                let end = val_to_f64(&self.stack.pop().unwrap(), "for")?;
                let start = val_to_f64(&self.stack.pop().unwrap(), "for")?;
                let ident = self.stack.pop().unwrap().borrow().to_string();
                if step == 0.0 {
                    return throw_string!("<for:zero-size step>");
                }
                set_scope_var(ident.clone(), new_value(VarValues::Num(start)), Gc::clone(&self.cur_scope));
                self.loop_stack.push(LoopFrame {
                    stack_vals: 0,
                    loop_data: LoopType::For {
                        ident,
                        value: start,
                        step,
                        end,
                    },
                });
            },
            Instruction::FORTEST(jump) => {
                match self.loop_stack.last().unwrap().loop_data {
                    LoopType::For {value, step, end, ..} => {
                        if step > 0.0 && value >= end {
                            *counter = *jump;
                            return Ok(());
                        } else if step < 0.0 && value <= end {
                            *counter = *jump;
                            return Ok(());
                        }
                    }
                    _ => {
                        panic!("invalid loop type in FORTEST");
                    }
                }
            },
            Instruction::FORITER => {
                match &mut self.loop_stack.last_mut().unwrap().loop_data {
                    LoopType::For {ident, value, step, ..} => {
                        *value += *step;
                        set_scope_var(ident.clone(), new_value(VarValues::Num(*value)), Gc::clone(&self.cur_scope));
                    }
                    _ => {
                        panic!("invalid loop type in FORTEST");
                    }
                }
            },
            Instruction::LOOPINCR => {
                self.loop_stack.last_mut().unwrap().stack_vals += 1;
            },
            Instruction::LOOPEND(output_val) => {
                if *output_val {
                    let n = self.loop_stack.pop().unwrap().stack_vals;
                    match n {
                        0 => {
                            self.stack.push(new_value(VarValues::Nil));
                        },
                        1 => {
                            // no concat necessary
                        },
                        _ => {
                            let concat_val = concat_vals(self.stack.split_off(self.stack.len() - n));
                            self.stack.push(concat_val);
                        },
                    }
                } else {
                    self.loop_stack.pop();
                }
            },
            Instruction::STARTCATCH(loc) => {
                let stack_size = self.stack.len();
                let loop_stack_size = self.loop_stack.len();
                *counter += 1;
                match self.catch_block(prog, outputter, counter) {
                    Ok(_) => {
                        let top_val = self.stack.pop().unwrap();
                        self.stack.push(
                            new_value(
                                VarValues::CatchResult(true, top_val)
                            )
                        );
                    },
                    Err(LangError::Throw(err_val)) => {
                        self.stack.truncate(stack_size);
                        self.loop_stack.truncate(loop_stack_size);
                        self.stack.push(
                            new_value(
                                VarValues::CatchResult(false, err_val)
                            )
                        );
                        *counter = *loc;
                        return Ok(());
                    },
                    Err(LangError::CatchUnwind(0)) => {
                        // unwind hit floor, no value is created, continue as normal
                    },
                    Err(LangError::CatchUnwind(n)) => {
                        // pass it along, catch_block() and interpret() handle this
                        return Err(LangError::CatchUnwind(n-1));
                    }
                }
            },
            Instruction::UNWINDCATCH(n) => {
                return Err(LangError::CatchUnwind(*n));
            }
            Instruction::THROWVAL => {
                let v = self.stack.pop().unwrap();
                return Err(LangError::Throw(v));
            },
            Instruction::END | Instruction::ENDCATCH => unimplemented!(),
        }
        *counter += 1;
        Ok(())
    }
    fn catch_block(&mut self, prog: &[Instruction], outputter: &mut dyn Outputter, counter: &mut usize) -> LangResult<()> {
        loop {
            //println!("stack: {:?}", self.stack);
            //println!("instr: {}, {:?}", *counter, prog[*counter]);
            match &prog[*counter] {
                Instruction::ENDCATCH => {
                    break;
                },
                Instruction::END => {
                    panic!("found end inside of catch block")
                }
                _ => {
                    match self.interpret_inst(prog, counter, outputter) {
                        Ok(()) => {}
                        Err(LangError::Throw(v)) => return Err(LangError::Throw(v)),
                        Err(LangError::CatchUnwind(0)) => return Err(LangError::CatchUnwind(0)),
                        Err(LangError::CatchUnwind(n)) => return Err(LangError::CatchUnwind(n-1)),
                    }
                }
            }
        }
        Ok(())
    }
    pub fn interpret(&mut self, prog: &[Instruction], outputter: &mut dyn Outputter) -> LangResult<()> {
        let mut counter = 0;
        loop {
            //println!("stack: {:?}", self.stack);
            //println!("instr: {}, {:?}", counter, prog[counter]);
            match &prog[counter] {
                Instruction::END => {
                    break;
                },
                Instruction::ENDCATCH => {
                    panic!("found endcatch outside of catch block");
                },
                _ => {
                    match self.interpret_inst(prog, &mut counter, outputter) {
                        Ok(()) => {}
                        Err(LangError::Throw(v)) => return Err(LangError::Throw(v)),
                        Err(LangError::CatchUnwind(_)) => {
                            // catch unwind is trying to unwind more catches than exist
                            panic!("catchunwind escaped outermost catch block");
                        },
                    }
                }
            }
        }
        Ok(())
    }
}
