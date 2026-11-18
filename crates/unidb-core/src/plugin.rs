//! Sandboxed WebAssembly (WASM) MVP binary parser and fuel metered stack VM.
//! Pure safe Rust (`unsafe_code = "forbid"`), bounded instructions, zero host syscalls.

use std::collections::HashMap;

use crate::error::{ErrorCategory, UniDbError};

const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6d];
const WASM_VERSION: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmFuncType {
    pub param_count: usize,
    pub result_count: usize,
}

#[derive(Debug, Clone)]
struct WasmFuncBody {
    extra_locals: usize,
    code: Vec<u8>,
}

/// Validated, sandboxed WASM plugin module ready for deterministic execution.
#[derive(Debug, Clone)]
pub struct WasmPluginModule {
    types: Vec<WasmFuncType>,
    functions: Vec<usize>,
    exports: HashMap<String, usize>,
    bodies: Vec<WasmFuncBody>,
}

impl WasmPluginModule {
    /// Parses and validates a WebAssembly MVP binary (`\0asm\x01\0\0\0`).
    pub fn parse(bytes: &[u8]) -> Result<Self, UniDbError> {
        if bytes.len() < 8 || bytes[0..4] != WASM_MAGIC {
            return Err(wasm_error(
                "plugin_invalid_magic",
                "Not a valid WebAssembly binary (missing \\0asm magic)",
            ));
        }
        if bytes[4..8] != WASM_VERSION {
            return Err(wasm_error(
                "plugin_unsupported_version",
                "Unsupported WebAssembly binary version",
            ));
        }

        let mut types = Vec::new();
        let mut functions = Vec::new();
        let mut exports = HashMap::new();
        let mut bodies = Vec::new();

        let mut cursor = 8usize;
        while cursor < bytes.len() {
            let section_id = bytes[cursor];
            cursor += 1;
            let size = read_u32_leb(bytes, &mut cursor)? as usize;
            let end = cursor.checked_add(size).ok_or_else(|| {
                wasm_error("plugin_corrupt_section", "WASM section size overflow")
            })?;
            if end > bytes.len() {
                return Err(wasm_error(
                    "plugin_truncated",
                    "WASM section extends past end of binary",
                ));
            }
            let payload = &bytes[cursor..end];
            match section_id {
                1 => types = parse_type_section(payload)?,
                3 => functions = parse_function_section(payload)?,
                7 => exports = parse_export_section(payload)?,
                10 => bodies = parse_code_section(payload)?,
                _ => {} // Ignore custom/memory/other sections cleanly.
            }
            cursor = end;
        }

        if functions.len() != bodies.len() {
            return Err(wasm_error(
                "plugin_mismatched_sections",
                "Function count does not match code body count",
            ));
        }
        for &type_idx in &functions {
            if type_idx >= types.len() {
                return Err(wasm_error(
                    "plugin_invalid_type",
                    "Function references missing type signature",
                ));
            }
        }
        for &func_idx in exports.values() {
            if func_idx >= functions.len() {
                return Err(wasm_error(
                    "plugin_invalid_export",
                    "Export references missing function index",
                ));
            }
        }

        Ok(Self {
            types,
            functions,
            exports,
            bodies,
        })
    }

    /// Lists exported function names in sorted order.
    pub fn export_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.exports.keys().cloned().collect();
        names.sort();
        names
    }

    /// Invokes an exported `i32` function inside the fuel metered sandbox.
    pub fn call_i32(
        &self,
        export_name: &str,
        args: &[i32],
        fuel_limit: usize,
    ) -> Result<i32, UniDbError> {
        let &func_idx = self.exports.get(export_name).ok_or_else(|| {
            wasm_error(
                "plugin_export_missing",
                format!("Export '{export_name}' not found in plugin"),
            )
        })?;
        let type_idx = self.functions[func_idx];
        let func_type = &self.types[type_idx];
        if args.len() != func_type.param_count {
            return Err(wasm_error(
                "plugin_arity_mismatch",
                format!(
                    "Function '{}' expects {} argument(s), got {}",
                    export_name,
                    func_type.param_count,
                    args.len()
                ),
            ));
        }
        let body = &self.bodies[func_idx];
        let mut locals = args.to_vec();
        locals.resize(args.len() + body.extra_locals, 0);

        execute_bytecode(&body.code, &mut locals, fuel_limit, func_type.result_count)
    }
}

fn execute_bytecode(
    code: &[u8],
    locals: &mut [i32],
    fuel_limit: usize,
    expected_results: usize,
) -> Result<i32, UniDbError> {
    let mut stack: Vec<i32> = Vec::new();
    let mut pc = 0usize;
    let mut fuel = fuel_limit;

    while pc < code.len() {
        if fuel == 0 {
            return Err(wasm_error(
                "plugin_fuel_exhausted",
                "Plugin exceeded instruction fuel limit",
            ));
        }
        fuel -= 1;

        let op = code[pc];
        pc += 1;
        match op {
            0x00 => {
                return Err(wasm_error(
                    "plugin_unreachable",
                    "Plugin trapped on unreachable instruction",
                ));
            }
            0x01 => {} // nop
            0x0b | 0x0f => break,
            0x1a => {
                pop(&mut stack)?;
            }
            0x20 => {
                let idx = read_u32_leb(code, &mut pc)? as usize;
                let val = *locals
                    .get(idx)
                    .ok_or_else(|| wasm_error("plugin_bad_local", "Invalid local index"))?;
                stack.push(val);
            }
            0x21 => {
                let idx = read_u32_leb(code, &mut pc)? as usize;
                let val = pop(&mut stack)?;
                let slot = locals
                    .get_mut(idx)
                    .ok_or_else(|| wasm_error("plugin_bad_local", "Invalid local index"))?;
                *slot = val;
            }
            0x22 => {
                let idx = read_u32_leb(code, &mut pc)? as usize;
                let val = *stack
                    .last()
                    .ok_or_else(|| wasm_error("plugin_stack_underflow", "Stack underflow"))?;
                let slot = locals
                    .get_mut(idx)
                    .ok_or_else(|| wasm_error("plugin_bad_local", "Invalid local index"))?;
                *slot = val;
            }
            0x41 => {
                let val = read_i32_leb(code, &mut pc)?;
                stack.push(val);
            }
            0x45 => {
                let a = pop(&mut stack)?;
                stack.push(i32::from(a == 0));
            }
            0x46 => bin_cmp(&mut stack, |a, b| a == b)?,
            0x47 => bin_cmp(&mut stack, |a, b| a != b)?,
            0x48 => bin_cmp(&mut stack, |a, b| a < b)?,
            0x4a => bin_cmp(&mut stack, |a, b| a > b)?,
            0x4c => bin_cmp(&mut stack, |a, b| a <= b)?,
            0x4e => bin_cmp(&mut stack, |a, b| a >= b)?,
            0x6a => bin_op(&mut stack, i32::wrapping_add)?,
            0x6b => bin_op(&mut stack, i32::wrapping_sub)?,
            0x6c => bin_op(&mut stack, i32::wrapping_mul)?,
            0x6d => {
                let b = pop(&mut stack)?;
                let a = pop(&mut stack)?;
                if b == 0 {
                    return Err(wasm_error("plugin_div_zero", "Integer divide by zero"));
                }
                let res = a.checked_div(b).ok_or_else(|| {
                    wasm_error("plugin_overflow", "Signed integer division overflow")
                })?;
                stack.push(res);
            }
            0x6f => {
                let b = pop(&mut stack)?;
                let a = pop(&mut stack)?;
                if b == 0 {
                    return Err(wasm_error("plugin_div_zero", "Integer remainder by zero"));
                }
                stack.push(a.wrapping_rem(b));
            }
            other => {
                return Err(wasm_error(
                    "plugin_unsupported_opcode",
                    format!("Unsupported WASM opcode 0x{other:02x}"),
                ));
            }
        }
    }

    if expected_results == 0 {
        return Ok(0);
    }
    pop(&mut stack)
}

fn pop(stack: &mut Vec<i32>) -> Result<i32, UniDbError> {
    stack
        .pop()
        .ok_or_else(|| wasm_error("plugin_stack_underflow", "WASM stack underflow"))
}

fn bin_op(stack: &mut Vec<i32>, op: fn(i32, i32) -> i32) -> Result<(), UniDbError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    stack.push(op(a, b));
    Ok(())
}

fn bin_cmp(stack: &mut Vec<i32>, cmp: fn(i32, i32) -> bool) -> Result<(), UniDbError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    stack.push(i32::from(cmp(a, b)));
    Ok(())
}

fn parse_type_section(payload: &[u8]) -> Result<Vec<WasmFuncType>, UniDbError> {
    let mut c = 0usize;
    let count = read_u32_leb(payload, &mut c)? as usize;
    let mut types = Vec::with_capacity(count);
    for _ in 0..count {
        let tag = *payload
            .get(c)
            .ok_or_else(|| wasm_error("plugin_truncated", "Unexpected EOF in type section"))?;
        c += 1;
        if tag != 0x60 {
            return Err(wasm_error("plugin_bad_type", "Expected functype tag 0x60"));
        }
        let param_count = read_u32_leb(payload, &mut c)? as usize;
        c = c
            .checked_add(param_count)
            .filter(|&end| end <= payload.len())
            .ok_or_else(|| wasm_error("plugin_truncated", "Truncated param types"))?;
        let result_count = read_u32_leb(payload, &mut c)? as usize;
        c = c
            .checked_add(result_count)
            .filter(|&end| end <= payload.len())
            .ok_or_else(|| wasm_error("plugin_truncated", "Truncated result types"))?;
        types.push(WasmFuncType {
            param_count,
            result_count,
        });
    }
    Ok(types)
}

fn parse_function_section(payload: &[u8]) -> Result<Vec<usize>, UniDbError> {
    let mut c = 0usize;
    let count = read_u32_leb(payload, &mut c)? as usize;
    let mut funcs = Vec::with_capacity(count);
    for _ in 0..count {
        funcs.push(read_u32_leb(payload, &mut c)? as usize);
    }
    Ok(funcs)
}

fn parse_export_section(payload: &[u8]) -> Result<HashMap<String, usize>, UniDbError> {
    let mut c = 0usize;
    let count = read_u32_leb(payload, &mut c)? as usize;
    let mut exports = HashMap::new();
    for _ in 0..count {
        let name_len = read_u32_leb(payload, &mut c)? as usize;
        let name_end = c
            .checked_add(name_len)
            .filter(|&end| end <= payload.len())
            .ok_or_else(|| wasm_error("plugin_truncated", "Truncated export name"))?;
        let name = std::str::from_utf8(&payload[c..name_end])
            .map_err(|_| wasm_error("plugin_bad_utf8", "Export name is not valid UTF-8"))?
            .to_string();
        c = name_end;
        let kind = *payload
            .get(c)
            .ok_or_else(|| wasm_error("plugin_truncated", "Truncated export kind"))?;
        c += 1;
        let index = read_u32_leb(payload, &mut c)? as usize;
        if kind == 0x00 {
            exports.insert(name, index);
        }
    }
    Ok(exports)
}

fn parse_code_section(payload: &[u8]) -> Result<Vec<WasmFuncBody>, UniDbError> {
    let mut c = 0usize;
    let count = read_u32_leb(payload, &mut c)? as usize;
    let mut bodies = Vec::with_capacity(count);
    for _ in 0..count {
        let body_size = read_u32_leb(payload, &mut c)? as usize;
        let body_end = c
            .checked_add(body_size)
            .filter(|&end| end <= payload.len())
            .ok_or_else(|| wasm_error("plugin_truncated", "Truncated code body"))?;
        let body_bytes = &payload[c..body_end];
        c = body_end;

        let mut bc = 0usize;
        let local_groups = read_u32_leb(body_bytes, &mut bc)? as usize;
        let mut extra_locals = 0usize;
        for _ in 0..local_groups {
            let group_count = read_u32_leb(body_bytes, &mut bc)? as usize;
            extra_locals = extra_locals.saturating_add(group_count);
            if bc >= body_bytes.len() {
                return Err(wasm_error("plugin_truncated", "Truncated local type"));
            }
            bc += 1;
        }
        bodies.push(WasmFuncBody {
            extra_locals,
            code: body_bytes[bc..].to_vec(),
        });
    }
    Ok(bodies)
}

fn read_u32_leb(bytes: &[u8], cursor: &mut usize) -> Result<u32, UniDbError> {
    let mut result: u32 = 0;
    let mut shift = 0u32;
    loop {
        let byte = *bytes
            .get(*cursor)
            .ok_or_else(|| wasm_error("plugin_truncated", "Unexpected EOF reading LEB128"))?;
        *cursor += 1;
        result |= u32::from(byte & 0x7f) << shift;
        if (byte & 0x80) == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 35 {
            return Err(wasm_error("plugin_bad_leb", "LEB128 integer overflow"));
        }
    }
}

fn read_i32_leb(bytes: &[u8], cursor: &mut usize) -> Result<i32, UniDbError> {
    let mut result: i32 = 0;
    let mut shift = 0u32;
    loop {
        let byte = *bytes.get(*cursor).ok_or_else(|| {
            wasm_error("plugin_truncated", "Unexpected EOF reading signed LEB128")
        })?;
        *cursor += 1;
        result |= i32::from(byte & 0x7f) << shift;
        shift += 7;
        if (byte & 0x80) == 0 {
            if shift < 32 && (byte & 0x40) != 0 {
                result |= !0 << shift;
            }
            return Ok(result);
        }
        if shift >= 35 {
            return Err(wasm_error("plugin_bad_leb", "Signed LEB128 overflow"));
        }
    }
}

fn wasm_error(code: &'static str, message: impl Into<String>) -> UniDbError {
    UniDbError::new(code, ErrorCategory::Unsupported, message)
}

/// Builds a compact valid WASM binary exporting `add(i32, i32) -> i32` and `score_bonus(i32) -> i32`
/// for built-in testing and sandbox demonstration.
pub fn sample_metric_plugin_wasm() -> Vec<u8> {
    let mut out = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    let push_section = |buf: &mut Vec<u8>, id: u8, payload: &[u8]| {
        buf.push(id);
        buf.push(payload.len() as u8);
        buf.extend_from_slice(payload);
    };
    // Section 1 (Type): (i32,i32)->i32 and (i32)->i32
    push_section(
        &mut out,
        1,
        &[
            0x02, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f, 0x60, 0x01, 0x7f, 0x01, 0x7f,
        ],
    );
    // Section 3 (Function): func 0 -> type 0, func 1 -> type 1
    push_section(&mut out, 3, &[0x02, 0x00, 0x01]);
    // Section 7 (Export): "add" -> 0, "score_bonus" -> 1
    push_section(
        &mut out,
        7,
        &[
            0x02, 0x03, b'a', b'd', b'd', 0x00, 0x00, 0x0b, b's', b'c', b'o', b'r', b'e', b'_',
            b'b', b'o', b'n', b'u', b's', 0x00, 0x01,
        ],
    );
    // Section 10 (Code): 2 bodies
    push_section(
        &mut out,
        10,
        &[
            0x02, // 2 bodies
            0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b, // add(a, b)
            0x0a, 0x00, 0x20, 0x00, 0x41, 0x02, 0x6c, 0x41, 0x0a, 0x6a, 0x0b, // x * 2 + 10
        ],
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_executes_real_wasm_binary() {
        let wasm = sample_metric_plugin_wasm();
        let module = WasmPluginModule::parse(&wasm).expect("parse valid wasm");
        assert_eq!(module.export_names(), vec!["add", "score_bonus"]);
        assert_eq!(module.call_i32("add", &[17, 25], 100).expect("add"), 42);
        assert_eq!(
            module.call_i32("score_bonus", &[15], 100).expect("bonus"),
            40
        );
    }

    #[test]
    fn traps_on_fuel_exhaustion_arity_and_bad_magic() {
        let wasm = sample_metric_plugin_wasm();
        let module = WasmPluginModule::parse(&wasm).expect("parse");

        let fuel_err = module
            .call_i32("add", &[1, 2], 2)
            .expect_err("exhaust fuel");
        assert_eq!(fuel_err.code(), "plugin_fuel_exhausted");

        let arity_err = module.call_i32("add", &[1], 100).expect_err("arity check");
        assert_eq!(arity_err.code(), "plugin_arity_mismatch");

        let bad_err = WasmPluginModule::parse(b"not-wasm").expect_err("bad magic");
        assert_eq!(bad_err.code(), "plugin_invalid_magic");
    }

    #[test]
    fn traps_safely_on_divide_by_zero() {
        // Minimal wasm exporting div(i32, i32) -> i32
        let wasm = vec![
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60, 0x02, 0x7f,
            0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03, b'd', b'i', b'v',
            0x00, 0x00, 0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6d, 0x0b,
        ];
        let module = WasmPluginModule::parse(&wasm).expect("parse");
        assert_eq!(module.call_i32("div", &[20, 4], 100).expect("div"), 5);
        let err = module.call_i32("div", &[20, 0], 100).expect_err("div0");
        assert_eq!(err.code(), "plugin_div_zero");
    }
}
