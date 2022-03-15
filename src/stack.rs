
use anyhow::{anyhow, Result};

use crate::ops::{Pos,Arg, UNINIT, NEG_SIGN, POS_SIGN};
use crate::parse::{skip_white, parse_arg};

// TODO: detect errors:
//  - bracket follows comma
//  - comma follows bracket
pub fn str_expr_to_vec(s: &str) -> Result<Vec<Arg>> {
    // let mut last_arg = Arg::Op(String::new());
    let mut args: Vec<Arg> = Vec::new();
    let mut st = skip_white(s);
    loop {
        let (st_in, arg) = parse_arg(st)?;
        if let Arg::End = arg {
            break;
        }
        st = skip_white(st_in);
        args.push(arg);
    }
    Ok(args)
}

// -> priority, right_assoc
fn priority(arg: &Arg) -> (u16, bool) {
    match arg {
        Arg::Op(s) => match s.as_str() {
            "+" | "-" => (5, false),
            "*" | "/" => (10, false),
            "^"  => (15, true),
            NEG_SIGN | POS_SIGN => (20, false),
            "&" => (5, false),
            "%" => (25, false),
            _ => panic!("unimplemented: {}", s),
        },
        Arg::Eq(_) => (1, false),
        _ => (0, false),
    }
}

fn is_bin_op(arg: &Arg) -> bool {
    match arg {
        Arg::Op(s) => match s.as_str() {
            "%" => false,
            _ => true,
        },
        Arg::Eq(_) | Arg::OBracket(_) => true,
        _ => false,
    }
}

fn lookup_func_arg(args: &[Arg], idx: usize) -> bool {
    let mut lvl = 0;
    let mut cnt = 0;
    for arg in args.iter().skip(idx) {
        match arg {
            Arg::OBracket(_) => lvl += 1,
            Arg::CBracket(_) => if lvl == 1 {
                return  false;
            } else if lvl == 0 {
                return false;
            } else {
                lvl -= 1;
            },
            Arg::Func(_,_) | Arg::Number(_) | Arg::Str(_) | Arg::Rng(_) => return true,
            _ => {},
        }
    }
    false
}

// TODO: detect errors
// Convert raw argument list to an easy to calculate vector
pub fn expr_to_stack(args: &[Arg]) -> Result<Vec<Arg>> {
    let mut is_last_op = true;
    let mut stack: Vec<Arg> = Vec::new();
    let mut expr: Vec<Arg> = Vec::new();

    for (idx, arg) in args.iter().enumerate() {
        match arg {
            Arg::OBracket(_) => {
                stack.push(arg.clone());
                is_last_op = true;
            },
            Arg::Number(_) | Arg::Str(_) | Arg::Rng(_) => {
                expr.push(arg.clone());
                is_last_op = false;
            },
            Arg::Bool(_) => {
                expr.push(arg.clone());
                is_last_op = false;
            },
            Arg::Func(name, _) => {
                let cnt = if lookup_func_arg(args, idx+1) { 1 } else { 0 };
                println!("{}: {} args", name, cnt);
                stack.push(Arg::Func(name.to_string(), cnt));
                is_last_op = false;
            },
            Arg::End => break,
            Arg::CBracket(b) => {
                loop {
                    match stack.pop() {
                        None => return Err(anyhow!("unmatched '{}'", b)),
                        Some(st) => match st {
                            Arg::OBracket(ref bb) => if (bb == "(" && b == ")") || (bb == "[" && b == "]") {
                                    break;
                                } else {
                                    return Err(anyhow!("brackets mismatch"));
                                },
                            _ => expr.push(st),
                        }
                    }
                }
                if let Some(s) = stack.last() {
                    if let Arg::Func(_, _) = s {
                        expr.push(s.clone());
                        stack.pop();
                    }
                }
                is_last_op = false;
            },
            Arg::Comma => {
                loop {
                    match stack.pop() {
                        None => return Err(anyhow!("comma outside brackets")),
                        Some(st) => match st {
                            Arg::OBracket(_) => {
                                let mut fn_arg = Arg::End;
                                if let Some(Arg::Func(name, arg_cnt)) = stack.last() {
                                    fn_arg = Arg::Func(name.to_string(), arg_cnt+1);
                                }
                                if fn_arg.is_func() {
                                    stack.pop();
                                    stack.push(fn_arg);
                                }
                                stack.push(st);
                                break;
                            },
                            aa @ _ => expr.push(aa.clone()),
                        }
                    }
                }
                is_last_op= true;
            },
            Arg::Eq(_) | Arg::Op(_) => {
                let arg = if is_last_op {
                    match arg {
                        Arg::Op(s) => match s.as_str() {
                            "-" => Arg::Op(String::from(NEG_SIGN)),
                            "+" => Arg::Op(String::from(POS_SIGN)),
                            _ => arg.clone(),
                        },
                        _ => arg.clone(),
                    }
                } else {
                    arg.clone()
                };
                is_last_op = is_bin_op(&arg);
                let (pri, right) = priority(&arg);
                loop {
                    match stack.last() {
                        None => {
                            stack.push(arg);
                            break;
                        },
                        Some(stk) => {
                            let (st_pri, _a) = priority(stk);
                            if st_pri > pri || (st_pri == pri && !right) {
                                expr.push(stk.clone()); // TODO: optimize?
                                stack.pop();
                            } else {
                                stack.push(arg);
                                break;
                            }
                        },
                    }
                }
            },
        }
    }
    for a in stack.drain(..).rev() {
        expr.push(a);
    }

    Ok(expr)
}

#[rustfmt::skip]
#[cfg(test)]
mod buf_test {
    use super::*;
    use crate::ops::*;
    #[test]
    fn build_expr() {
        struct Tst {
            val: &'static str,
            res: Vec<Arg>,
            err: bool,
        }
        let tests: Vec<Tst> = vec![
            Tst{
                val: "sum(sin(),cos(1,3))", err: false,
                res: Vec::new(), // TODO:
            },
            Tst{
                val: "sum(-234+A5,57)*20%-8", err: false,
                res: Vec::new(), // TODO:
            },
        ];
        for t in tests {
            let s = str_expr_to_vec(t.val).unwrap();
            let r = expr_to_stack(&s);
            if t.err {
                if r.is_ok() {
                    println!("response for [{}]: {:?}", t.val, r);
                }
                assert!(r.is_err(), "{}", t.val);
            } else {
                assert_eq!(r.unwrap(), t.res, "[{}]", t.val);
            }
        }
    }
}
