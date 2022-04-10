use std::collections::HashMap;
use std::ops::Bound::Included;

use anyhow::{anyhow, Result};

use crate::ops::{Arg, NEG_SIGN, POS_SIGN, pos_to_id, id_to_pos};
use crate::sheet::{Sheet};
use crate::stack::{str_expr_to_vec, expr_to_stack};

pub struct Expr {
    stk: Vec<Arg>,
    pub cache: HashMap<u64, u8>,
}

impl Default for Expr {
    fn default() -> Expr {
        Expr { cache: HashMap::new(), stk: Vec::new(), }
    }
}

impl Expr {
    pub fn calculate(&mut self, args: &[Arg], sheet: &mut Sheet) -> Result<Arg> {
        for arg in args {
            match arg {
                Arg::Number(_) | Arg::Bool(_) | Arg::Str(_) | Arg::Rng(_) => {
                    self.stk.push(arg.clone());
                    continue;
                },
                Arg::Op(op) => self.calc_op(op, sheet)?,
                Arg::Eq(eq) => self.calc_condition(eq, sheet)?,
                Arg::Func(nm, cnt) => self.calc_func(nm, *cnt, sheet)?,
                _ => unreachable!("{:?}", arg),
            }
        }
        if self.stk.len() != 1 {
            return Err(anyhow!("invalid expression"));
        }
        if let Some(a) = self.stk.pop() {
            self.single_cell(sheet, a)
        } else {
            unreachable!("invalid expression")
        }
    }

    fn single_cell(&mut self, sheet: &mut Sheet, arg: Arg) -> Result<Arg> {
        match arg {
            Arg::Rng(ref v) => {
                info!("single cell: {:?}", v);
                if v.len() != 1 {
                    return Err(anyhow!("cannot get a cell from a range {:?}", arg));
                }
                let mut cell = sheet.cell(v[0].col, v[0].row);
                if !cell.is_expr() {
                    return Ok(cell.calculated.clone());
                }
                let uid = pos_to_id(v[0].col, v[0].row);
                let state = match self.cache.get(&uid) {
                    None => 0,
                    Some(v) => *v,
                };
                if state == 1 {
                    return Err(anyhow!("recursion"));
                } else if state == 0 {
                    self.cache.insert(uid, 1);
                    let args = str_expr_to_vec(&cell.val[1..])?;
                    let args = expr_to_stack(&args)?;
                    let res = self.calculate(&args, sheet);
                    sheet.set_cell_calc_value(v[0].col, v[0].row, res);
                    self.cache.insert(uid, 2);
                    cell = sheet.cell(v[0].col, v[0].row);
                }

                if cell.err != 0 {
                    Err(anyhow!("invalid formula in {:?}", v[0]))
                } else {
                    Ok(cell.calculated.clone())
                }
            },
            _ => Ok(arg),
        }
    }

    fn calc_op(&mut self, op: &str, sheet: &mut Sheet) -> Result<()> {
        match op {
            NEG_SIGN => {
                let arg = self.stk.pop().ok_or(anyhow!("empty stack"))?;
                let arg = self.single_cell(sheet, arg)?;
                let f = try_to_num(arg)?;
                self.stk.push(Arg::Number(-f));
            },
            POS_SIGN => {
                let arg = self.stk.pop().ok_or(anyhow!("empty stack"))?;
                let arg = self.single_cell(sheet, arg)?;
                let f = try_to_num(arg)?;
                self.stk.push(Arg::Number(f));
            },
            "%" => {
                let arg = self.stk.pop().ok_or(anyhow!("empty stack"))?;
                let arg = self.single_cell(sheet, arg)?;
                let f = try_to_num(arg)?;
                self.stk.push(Arg::Number(f/100.0));
            },
            "*" => {
                let arg1 = self.stk.pop().ok_or(anyhow!("empty stack"))?;
                let arg1 = self.single_cell(sheet, arg1)?;
                let arg2 = self.stk.pop().ok_or(anyhow!("empty stack"))?;
                let arg2 = self.single_cell(sheet, arg2)?;
                let f1 = try_to_num(arg1)?;
                let f2 = try_to_num(arg2)?;
                self.stk.push(Arg::Number(f1*f2));
            },
            "/" => {
                let arg1 = self.stk.pop().ok_or(anyhow!("empty stack"))?;
                let arg1 = self.single_cell(sheet, arg1)?;
                let arg2 = self.stk.pop().ok_or(anyhow!("empty stack"))?;
                let arg2 = self.single_cell(sheet, arg2)?;
                let f1 = try_to_num(arg1)?;
                if f1 == 0.0 {
                    return Err(anyhow!("division by zero"));
                }
                let f2 = try_to_num(arg2)?;
                self.stk.push(Arg::Number(f2/f1));
            },
            "+" => {
                // TODO: dates
                let arg1 = self.stk.pop().ok_or(anyhow!("empty stack"))?;
                let arg1 = self.single_cell(sheet, arg1)?;
                let arg2 = self.stk.pop().ok_or(anyhow!("empty stack"))?;
                let arg2 = self.single_cell(sheet, arg2)?;
                let f1 = try_to_num(arg1)?;
                let f2 = try_to_num(arg2)?;
                self.stk.push(Arg::Number(f1+f2));
            },
            "-" => {
                // TODO: dates
                let arg1 = self.stk.pop().ok_or(anyhow!("empty stack"))?;
                let arg1 = self.single_cell(sheet, arg1)?;
                let arg2 = self.stk.pop().ok_or(anyhow!("empty stack"))?;
                let arg2 = self.single_cell(sheet, arg2)?;
                let f1 = try_to_num(arg1)?;
                let f2 = try_to_num(arg2)?;
                self.stk.push(Arg::Number(f2-f1));
            },
            "^" => {
                let arg1 = self.stk.pop().ok_or(anyhow!("empty stack"))?;
                let arg1 = self.single_cell(sheet, arg1)?;
                let arg2 = self.stk.pop().ok_or(anyhow!("empty stack"))?;
                let arg2 = self.single_cell(sheet, arg2)?;
                let f1 = try_to_num(arg1)?;
                let f2 = try_to_num(arg2)?;
                self.stk.push(Arg::Number(f2.powf(f1)));
            },
            "&" => {
                let arg1 = self.stk.pop().ok_or(anyhow!("empty stack"))?;
                let arg1 = self.single_cell(sheet, arg1)?;
                let arg2 = self.stk.pop().ok_or(anyhow!("empty stack"))?;
                let arg2 = self.single_cell(sheet, arg2)?;
                let f1 = try_to_str(&arg1)?;
                let f2 = try_to_str(&arg2)?;
                self.stk.push(Arg::Str(f1+&f2));
            },
            _ => return Err(anyhow!("invalid operator {}", op)),
        }
        Ok(())
    }
    fn calc_condition(&mut self, eq: &str, sheet: &mut Sheet) -> Result<()> {
        let arg1 = self.stk.pop().ok_or(anyhow!("empty stack"))?;
        let arg1 = self.single_cell(sheet, arg1)?;
        let arg2 = self.stk.pop().ok_or(anyhow!("empty stack"))?;
        let arg2 = self.single_cell(sheet, arg2)?;
        match eq {
            "<>" | "!=" => {
                let b = eq_op(&arg1, &arg2)?;
                self.stk.push(Arg::Bool(!b));
            },
            "=" | "==" => {
                let b = eq_op(&arg1, &arg2)?;
                self.stk.push(Arg::Bool(b));
            },
            "<" => {
                let b = less_op(&arg2, &arg1)?;
                self.stk.push(Arg::Bool(b));
            },
            ">=" => {
                let b = less_op(&arg2, &arg1)?;
                self.stk.push(Arg::Bool(!b));
            },
            ">" => {
                let b = greater_op(&arg2, &arg1)?;
                self.stk.push(Arg::Bool(b));
            },
            "<=" => {
                let b = greater_op(&arg2, &arg1)?;
                self.stk.push(Arg::Bool(!b));
            },
            _ => return Err(anyhow!("invalid equality operator {}", eq)),
        }
        Ok(())
    }
    fn calc_func(&mut self, name: &str, cnt: usize, sheet: &mut Sheet) -> Result<()> {
        match name.to_lowercase().as_str() {
            "sum" => self.sum(cnt, sheet),
            _ => Err(anyhow!("unimplemented")),
        }
    }
    fn sum(&mut self, cnt: usize, sheet: &mut Sheet) -> Result<()> {
        if cnt == 0 {
            return Err(anyhow!("SUM requires at least one argument"));
        }
        let mut sum: f64 = 0.0;
        for _i in 0..cnt {
            let arg = self.stk.pop().ok_or(anyhow!("empty stack"))?;
            match arg {
                Arg::Rng(v) => {
                    let (start_col, start_row, end_col, end_row) = if v.len() == 1 {
                        (v[0].col, v[0].row, v[0].col, v[0].row)
                    } else {
                        (v[0].col, v[0].row, v[1].col, v[1].row)
                    };
                    let st_id = pos_to_id(start_col, start_row);
                    let en_id = pos_to_id(end_col, end_row);
                    for (&id, val) in sheet.cells.range((Included(&st_id), Included(&en_id))) {
                        let (col, _row) = id_to_pos(id);
                        if col < start_col || col > end_col {
                            continue;
                        }
                        if let Ok(n) = try_to_num(val.calculated.clone()) {
                            sum += n;
                        }
                    }
                },
                _ => if let Ok(n) = try_to_num(arg) {
                    sum += n;
                },
            }
        }
        self.stk.push(Arg::Number(sum));
        Ok(())
    }
}

fn eq_op(a: &Arg, b: &Arg) -> Result<bool> {
    match (a, b) {
        (Arg::Number(na), Arg::Number(nb)) => Ok(na == nb),
        (Arg::Bool(na), Arg::Bool(nb)) => Ok(na == nb),
        (Arg::Str(s), _) | (_, Arg::Str(s)) => {
            let sa = try_to_str(a)?;
            let sb = try_to_str(b)?;
            Ok(sa == sb)
        },
        _ => unreachable!("must be unimplemented for {:?} and {:?}", a, b),
    }
}

fn greater_op(a: &Arg, b: &Arg) -> Result<bool> {
    match (a, b) {
        (Arg::Number(na), Arg::Number(nb)) => Ok(na > nb),
        (Arg::Bool(na), Arg::Bool(nb)) => Ok(na > nb),
        (Arg::Str(s), _) | (_, Arg::Str(s)) => {
            let sa = try_to_str(a)?;
            let sb = try_to_str(b)?;
            Ok(sa > sb)
        },
        _ => unreachable!("must be unimplemented for {:?} and {:?}", a, b),
    }
}

fn less_op(a: &Arg, b: &Arg) -> Result<bool> {
    match (a, b) {
        (Arg::Number(na), Arg::Number(nb)) => Ok(na < nb),
        (Arg::Bool(na), Arg::Bool(nb)) => Ok(na < nb),
        (Arg::Str(s), _) | (_, Arg::Str(s)) => {
            let sa = try_to_str(a)?;
            let sb = try_to_str(b)?;
            Ok(sa < sb)
        },
        _ => unreachable!("must be unimplemented for {:?} and {:?}", a, b),
    }
}

fn try_to_num(a: Arg) -> Result<f64> {
    match a {
        Arg::End => return Ok(0.0),
        Arg::Str(s) => {
            if s.is_empty() {
                return Ok(0.0);
            }
            if let Ok(f) = s.parse::<f64>() {
                Ok(f)
            } else {
                Err(anyhow!("cannot convert {} to a number", s))
            }
        },
        Arg::Number(n) => Ok(n),
        Arg::Bool(b) => Ok(if b { 1.0 } else { 0.0 }),
        _ => Err(anyhow!("faled to convert to a number")),
    }
}
fn try_to_str(a: &Arg) -> Result<String> {
    match a {
        Arg::End => Ok(String::new()),
        Arg::Str(s) => Ok(s.to_string()),
        Arg::Number(n) => Ok(n.to_string()),
        Arg::Bool(b) => Ok(if *b { "true".to_string() } else { "false".to_string() }),
        _ => Err(anyhow!("faled to convert to a string")),
    }
}
fn try_to_bool(a: Arg) -> Result<bool> {
    match a {
        Arg::End => Ok(false),
        Arg::Str(s) => Ok(!s.is_empty()),
        Arg::Number(n) => Ok(n != 0.0),
        Arg::Bool(b) => Ok(b),
        _ => Err(anyhow!("faled to convert to a string")),
    }
}
