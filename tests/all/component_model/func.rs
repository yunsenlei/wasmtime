use super::REALLOC_AND_FREE;
use anyhow::Result;
use std::rc::Rc;
use std::sync::Arc;
use wasmtime::component::*;
use wasmtime::{Store, Trap, TrapCode};

const CANON_32BIT_NAN: u32 = 0b01111111110000000000000000000000;
const CANON_64BIT_NAN: u64 = 0b0111111111111000000000000000000000000000000000000000000000000000;

#[test]
fn thunks() -> Result<()> {
    let component = r#"
        (component
            (core module $m
                (func (export "thunk"))
                (func (export "thunk-trap") unreachable)
            )
            (core instance $i (instantiate $m))
            (func (export "thunk")
                (canon lift (core func $i "thunk"))
            )
            (func (export "thunk-trap")
                (canon lift (core func $i "thunk-trap"))
            )
        )
    "#;

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;
    instance
        .get_typed_func::<(), (), _>(&mut store, "thunk")?
        .call(&mut store, ())?;
    let err = instance
        .get_typed_func::<(), (), _>(&mut store, "thunk-trap")?
        .call(&mut store, ())
        .unwrap_err();
    assert!(err.downcast::<Trap>()?.trap_code() == Some(TrapCode::UnreachableCodeReached));

    Ok(())
}

#[test]
fn typecheck() -> Result<()> {
    let component = r#"
        (component
            (core module $m
                (func (export "thunk"))
                (func (export "take-string") (param i32 i32))
                (func (export "two-args") (param i32 i32 i32))
                (func (export "ret-one") (result i32) unreachable)

                (memory (export "memory") 1)
                (func (export "realloc") (param i32 i32 i32 i32) (result i32)
                    unreachable)
            )
            (core instance $i (instantiate (module $m)))
            (func (export "thunk")
                (canon lift (core func $i "thunk"))
            )
            (func (export "tuple-thunk") (param (tuple)) (result (tuple))
                (canon lift (core func $i "thunk"))
            )
            (func (export "take-string") (param string)
                (canon lift (core func $i "take-string") (memory $i "memory") (realloc (func $i "realloc")))
            )
            (func (export "take-two-args") (param s32) (param (list u8))
                (canon lift (core func $i "two-args") (memory $i "memory") (realloc (func $i "realloc")))
            )
            (func (export "ret-tuple") (result (tuple u8 s8))
                (canon lift (core func $i "ret-one") (memory $i "memory") (realloc (func $i "realloc")))
            )
            (func (export "ret-tuple1") (result (tuple u32))
                (canon lift (core func $i "ret-one") (memory $i "memory") (realloc (func $i "realloc")))
            )
            (func (export "ret-string") (result string)
                (canon lift (core func $i "ret-one") (memory $i "memory") (realloc (func $i "realloc")))
            )
            (func (export "ret-list-u8") (result (list u8))
                (canon lift (core func $i "ret-one") (memory $i "memory") (realloc (func $i "realloc")))
            )
        )
    "#;

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;
    let thunk = instance.get_func(&mut store, "thunk").unwrap();
    let tuple_thunk = instance.get_func(&mut store, "tuple-thunk").unwrap();
    let take_string = instance.get_func(&mut store, "take-string").unwrap();
    let take_two_args = instance.get_func(&mut store, "take-two-args").unwrap();
    let ret_tuple = instance.get_func(&mut store, "ret-tuple").unwrap();
    let ret_tuple1 = instance.get_func(&mut store, "ret-tuple1").unwrap();
    let ret_string = instance.get_func(&mut store, "ret-string").unwrap();
    let ret_list_u8 = instance.get_func(&mut store, "ret-list-u8").unwrap();
    assert!(thunk.typed::<(), u32, _>(&store).is_err());
    assert!(thunk.typed::<(u32,), (), _>(&store).is_err());
    assert!(thunk.typed::<(), (), _>(&store).is_ok());
    assert!(tuple_thunk.typed::<(), (), _>(&store).is_err());
    assert!(tuple_thunk.typed::<((),), (), _>(&store).is_ok());
    assert!(take_string.typed::<(), (), _>(&store).is_err());
    assert!(take_string.typed::<(String,), (), _>(&store).is_ok());
    assert!(take_string.typed::<(&str,), (), _>(&store).is_ok());
    assert!(take_string.typed::<(&[u8],), (), _>(&store).is_err());
    assert!(take_two_args.typed::<(), (), _>(&store).is_err());
    assert!(take_two_args.typed::<(i32, &[u8]), u32, _>(&store).is_err());
    assert!(take_two_args.typed::<(u32, &[u8]), (), _>(&store).is_err());
    assert!(take_two_args.typed::<(i32, &[u8]), (), _>(&store).is_ok());
    assert!(ret_tuple.typed::<(), (), _>(&store).is_err());
    assert!(ret_tuple.typed::<(), (u8,), _>(&store).is_err());
    assert!(ret_tuple.typed::<(), (u8, i8), _>(&store).is_ok());
    assert!(ret_tuple1.typed::<(), (u32,), _>(&store).is_ok());
    assert!(ret_tuple1.typed::<(), u32, _>(&store).is_err());
    assert!(ret_string.typed::<(), (), _>(&store).is_err());
    assert!(ret_string.typed::<(), WasmStr, _>(&store).is_ok());
    assert!(ret_list_u8.typed::<(), WasmList<u16>, _>(&store).is_err());
    assert!(ret_list_u8.typed::<(), WasmList<i8>, _>(&store).is_err());
    assert!(ret_list_u8.typed::<(), WasmList<u8>, _>(&store).is_ok());

    Ok(())
}

#[test]
fn integers() -> Result<()> {
    let component = r#"
        (component
            (core module $m
                (func (export "take-i32-100") (param i32)
                    local.get 0
                    i32.const 100
                    i32.eq
                    br_if 0
                    unreachable
                )
                (func (export "take-i64-100") (param i64)
                    local.get 0
                    i64.const 100
                    i64.eq
                    br_if 0
                    unreachable
                )
                (func (export "ret-i32-0") (result i32) i32.const 0)
                (func (export "ret-i64-0") (result i64) i64.const 0)
                (func (export "ret-i32-minus-1") (result i32) i32.const -1)
                (func (export "ret-i64-minus-1") (result i64) i64.const -1)
                (func (export "ret-i32-100000") (result i32) i32.const 100000)
            )
            (core instance $i (instantiate (module $m)))
            (func (export "take-u8") (param u8) (canon lift (core func $i "take-i32-100")))
            (func (export "take-s8") (param s8) (canon lift (core func $i "take-i32-100")))
            (func (export "take-u16") (param u16) (canon lift (core func $i "take-i32-100")))
            (func (export "take-s16") (param s16) (canon lift (core func $i "take-i32-100")))
            (func (export "take-u32") (param u32) (canon lift (core func $i "take-i32-100")))
            (func (export "take-s32") (param s32) (canon lift (core func $i "take-i32-100")))
            (func (export "take-u64") (param u64) (canon lift (core func $i "take-i64-100")))
            (func (export "take-s64") (param s64) (canon lift (core func $i "take-i64-100")))

            (func (export "ret-u8") (result u8) (canon lift (core func $i "ret-i32-0")))
            (func (export "ret-s8") (result s8) (canon lift (core func $i "ret-i32-0")))
            (func (export "ret-u16") (result u16) (canon lift (core func $i "ret-i32-0")))
            (func (export "ret-s16") (result s16) (canon lift (core func $i "ret-i32-0")))
            (func (export "ret-u32") (result u32) (canon lift (core func $i "ret-i32-0")))
            (func (export "ret-s32") (result s32) (canon lift (core func $i "ret-i32-0")))
            (func (export "ret-u64") (result u64) (canon lift (core func $i "ret-i64-0")))
            (func (export "ret-s64") (result s64) (canon lift (core func $i "ret-i64-0")))

            (func (export "retm1-u8") (result u8) (canon lift (core func $i "ret-i32-minus-1")))
            (func (export "retm1-s8") (result s8) (canon lift (core func $i "ret-i32-minus-1")))
            (func (export "retm1-u16") (result u16) (canon lift (core func $i "ret-i32-minus-1")))
            (func (export "retm1-s16") (result s16) (canon lift (core func $i "ret-i32-minus-1")))
            (func (export "retm1-u32") (result u32) (canon lift (core func $i "ret-i32-minus-1")))
            (func (export "retm1-s32") (result s32) (canon lift (core func $i "ret-i32-minus-1")))
            (func (export "retm1-u64") (result u64) (canon lift (core func $i "ret-i64-minus-1")))
            (func (export "retm1-s64") (result s64) (canon lift (core func $i "ret-i64-minus-1")))

            (func (export "retbig-u8") (result u8) (canon lift (core func $i "ret-i32-100000")))
            (func (export "retbig-s8") (result s8) (canon lift (core func $i "ret-i32-100000")))
            (func (export "retbig-u16") (result u16) (canon lift (core func $i "ret-i32-100000")))
            (func (export "retbig-s16") (result s16) (canon lift (core func $i "ret-i32-100000")))
            (func (export "retbig-u32") (result u32) (canon lift (core func $i "ret-i32-100000")))
            (func (export "retbig-s32") (result s32) (canon lift (core func $i "ret-i32-100000")))
        )
    "#;

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;

    // Passing in 100 is valid for all primitives
    instance
        .get_typed_func::<(u8,), (), _>(&mut store, "take-u8")?
        .call(&mut store, (100,))?;
    instance
        .get_typed_func::<(i8,), (), _>(&mut store, "take-s8")?
        .call(&mut store, (100,))?;
    instance
        .get_typed_func::<(u16,), (), _>(&mut store, "take-u16")?
        .call(&mut store, (100,))?;
    instance
        .get_typed_func::<(i16,), (), _>(&mut store, "take-s16")?
        .call(&mut store, (100,))?;
    instance
        .get_typed_func::<(u32,), (), _>(&mut store, "take-u32")?
        .call(&mut store, (100,))?;
    instance
        .get_typed_func::<(i32,), (), _>(&mut store, "take-s32")?
        .call(&mut store, (100,))?;
    instance
        .get_typed_func::<(u64,), (), _>(&mut store, "take-u64")?
        .call(&mut store, (100,))?;
    instance
        .get_typed_func::<(i64,), (), _>(&mut store, "take-s64")?
        .call(&mut store, (100,))?;

    // This specific wasm instance traps if any value other than 100 is passed
    instance
        .get_typed_func::<(u8,), (), _>(&mut store, "take-u8")?
        .call(&mut store, (101,))
        .unwrap_err()
        .downcast::<Trap>()?;
    instance
        .get_typed_func::<(i8,), (), _>(&mut store, "take-s8")?
        .call(&mut store, (101,))
        .unwrap_err()
        .downcast::<Trap>()?;
    instance
        .get_typed_func::<(u16,), (), _>(&mut store, "take-u16")?
        .call(&mut store, (101,))
        .unwrap_err()
        .downcast::<Trap>()?;
    instance
        .get_typed_func::<(i16,), (), _>(&mut store, "take-s16")?
        .call(&mut store, (101,))
        .unwrap_err()
        .downcast::<Trap>()?;
    instance
        .get_typed_func::<(u32,), (), _>(&mut store, "take-u32")?
        .call(&mut store, (101,))
        .unwrap_err()
        .downcast::<Trap>()?;
    instance
        .get_typed_func::<(i32,), (), _>(&mut store, "take-s32")?
        .call(&mut store, (101,))
        .unwrap_err()
        .downcast::<Trap>()?;
    instance
        .get_typed_func::<(u64,), (), _>(&mut store, "take-u64")?
        .call(&mut store, (101,))
        .unwrap_err()
        .downcast::<Trap>()?;
    instance
        .get_typed_func::<(i64,), (), _>(&mut store, "take-s64")?
        .call(&mut store, (101,))
        .unwrap_err()
        .downcast::<Trap>()?;

    // Zero can be returned as any integer
    assert_eq!(
        instance
            .get_typed_func::<(), u8, _>(&mut store, "ret-u8")?
            .call(&mut store, ())?,
        0
    );
    assert_eq!(
        instance
            .get_typed_func::<(), i8, _>(&mut store, "ret-s8")?
            .call(&mut store, ())?,
        0
    );
    assert_eq!(
        instance
            .get_typed_func::<(), u16, _>(&mut store, "ret-u16")?
            .call(&mut store, ())?,
        0
    );
    assert_eq!(
        instance
            .get_typed_func::<(), i16, _>(&mut store, "ret-s16")?
            .call(&mut store, ())?,
        0
    );
    assert_eq!(
        instance
            .get_typed_func::<(), u32, _>(&mut store, "ret-u32")?
            .call(&mut store, ())?,
        0
    );
    assert_eq!(
        instance
            .get_typed_func::<(), i32, _>(&mut store, "ret-s32")?
            .call(&mut store, ())?,
        0
    );
    assert_eq!(
        instance
            .get_typed_func::<(), u64, _>(&mut store, "ret-u64")?
            .call(&mut store, ())?,
        0
    );
    assert_eq!(
        instance
            .get_typed_func::<(), i64, _>(&mut store, "ret-s64")?
            .call(&mut store, ())?,
        0
    );

    // Returning -1 should reinterpret the bytes as defined by each type.
    assert_eq!(
        instance
            .get_typed_func::<(), u8, _>(&mut store, "retm1-u8")?
            .call(&mut store, ())?,
        0xff
    );
    assert_eq!(
        instance
            .get_typed_func::<(), i8, _>(&mut store, "retm1-s8")?
            .call(&mut store, ())?,
        -1
    );
    assert_eq!(
        instance
            .get_typed_func::<(), u16, _>(&mut store, "retm1-u16")?
            .call(&mut store, ())?,
        0xffff
    );
    assert_eq!(
        instance
            .get_typed_func::<(), i16, _>(&mut store, "retm1-s16")?
            .call(&mut store, ())?,
        -1
    );
    assert_eq!(
        instance
            .get_typed_func::<(), u32, _>(&mut store, "retm1-u32")?
            .call(&mut store, ())?,
        0xffffffff
    );
    assert_eq!(
        instance
            .get_typed_func::<(), i32, _>(&mut store, "retm1-s32")?
            .call(&mut store, ())?,
        -1
    );
    assert_eq!(
        instance
            .get_typed_func::<(), u64, _>(&mut store, "retm1-u64")?
            .call(&mut store, ())?,
        0xffffffff_ffffffff
    );
    assert_eq!(
        instance
            .get_typed_func::<(), i64, _>(&mut store, "retm1-s64")?
            .call(&mut store, ())?,
        -1
    );

    // Returning 100000 should chop off bytes as necessary
    let ret: u32 = 100000;
    assert_eq!(
        instance
            .get_typed_func::<(), u8, _>(&mut store, "retbig-u8")?
            .call(&mut store, ())?,
        ret as u8,
    );
    assert_eq!(
        instance
            .get_typed_func::<(), i8, _>(&mut store, "retbig-s8")?
            .call(&mut store, ())?,
        ret as i8,
    );
    assert_eq!(
        instance
            .get_typed_func::<(), u16, _>(&mut store, "retbig-u16")?
            .call(&mut store, ())?,
        ret as u16,
    );
    assert_eq!(
        instance
            .get_typed_func::<(), i16, _>(&mut store, "retbig-s16")?
            .call(&mut store, ())?,
        ret as i16,
    );
    assert_eq!(
        instance
            .get_typed_func::<(), u32, _>(&mut store, "retbig-u32")?
            .call(&mut store, ())?,
        ret,
    );
    assert_eq!(
        instance
            .get_typed_func::<(), i32, _>(&mut store, "retbig-s32")?
            .call(&mut store, ())?,
        ret as i32,
    );

    Ok(())
}

#[test]
fn type_layers() -> Result<()> {
    let component = r#"
        (component
            (core module $m
                (func (export "take-i32-100") (param i32)
                    local.get 0
                    i32.const 2
                    i32.eq
                    br_if 0
                    unreachable
                )
            )
            (core instance $i (instantiate $m))
            (func (export "take-u32") (param u32) (canon lift (core func $i "take-i32-100")))
        )
    "#;

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;

    instance
        .get_typed_func::<(Box<u32>,), (), _>(&mut store, "take-u32")?
        .call(&mut store, (Box::new(2),))?;
    instance
        .get_typed_func::<(&u32,), (), _>(&mut store, "take-u32")?
        .call(&mut store, (&2,))?;
    instance
        .get_typed_func::<(Rc<u32>,), (), _>(&mut store, "take-u32")?
        .call(&mut store, (Rc::new(2),))?;
    instance
        .get_typed_func::<(Arc<u32>,), (), _>(&mut store, "take-u32")?
        .call(&mut store, (Arc::new(2),))?;
    instance
        .get_typed_func::<(&Box<Arc<Rc<u32>>>,), (), _>(&mut store, "take-u32")?
        .call(&mut store, (&Box::new(Arc::new(Rc::new(2))),))?;

    Ok(())
}

#[test]
fn floats() -> Result<()> {
    let component = r#"
        (component
            (core module $m
                (func (export "i32.reinterpret_f32") (param f32) (result i32)
                    local.get 0
                    i32.reinterpret_f32
                )
                (func (export "i64.reinterpret_f64") (param f64) (result i64)
                    local.get 0
                    i64.reinterpret_f64
                )
                (func (export "f32.reinterpret_i32") (param i32) (result f32)
                    local.get 0
                    f32.reinterpret_i32
                )
                (func (export "f64.reinterpret_i64") (param i64) (result f64)
                    local.get 0
                    f64.reinterpret_i64
                )
            )
            (core instance $i (instantiate $m))

            (func (export "f32-to-u32") (param float32) (result u32)
                (canon lift (core func $i "i32.reinterpret_f32"))
            )
            (func (export "f64-to-u64") (param float64) (result u64)
                (canon lift (core func $i "i64.reinterpret_f64"))
            )
            (func (export "u32-to-f32") (param u32) (result float32)
                (canon lift (core func $i "f32.reinterpret_i32"))
            )
            (func (export "u64-to-f64") (param u64) (result float64)
                (canon lift (core func $i "f64.reinterpret_i64"))
            )
        )
    "#;

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;
    let f32_to_u32 = instance.get_typed_func::<(f32,), u32, _>(&mut store, "f32-to-u32")?;
    let f64_to_u64 = instance.get_typed_func::<(f64,), u64, _>(&mut store, "f64-to-u64")?;
    let u32_to_f32 = instance.get_typed_func::<(u32,), f32, _>(&mut store, "u32-to-f32")?;
    let u64_to_f64 = instance.get_typed_func::<(u64,), f64, _>(&mut store, "u64-to-f64")?;

    assert_eq!(f32_to_u32.call(&mut store, (1.0,))?, 1.0f32.to_bits());
    assert_eq!(f64_to_u64.call(&mut store, (2.0,))?, 2.0f64.to_bits());
    assert_eq!(u32_to_f32.call(&mut store, (3.0f32.to_bits(),))?, 3.0);
    assert_eq!(u64_to_f64.call(&mut store, (4.0f64.to_bits(),))?, 4.0);

    assert_eq!(
        u32_to_f32
            .call(&mut store, (CANON_32BIT_NAN | 1,))?
            .to_bits(),
        CANON_32BIT_NAN
    );
    assert_eq!(
        u64_to_f64
            .call(&mut store, (CANON_64BIT_NAN | 1,))?
            .to_bits(),
        CANON_64BIT_NAN
    );

    assert_eq!(
        f32_to_u32.call(&mut store, (f32::from_bits(CANON_32BIT_NAN | 1),))?,
        CANON_32BIT_NAN
    );
    assert_eq!(
        f64_to_u64.call(&mut store, (f64::from_bits(CANON_64BIT_NAN | 1),))?,
        CANON_64BIT_NAN
    );

    Ok(())
}

#[test]
fn bools() -> Result<()> {
    let component = r#"
        (component
            (core module $m
                (func (export "pass") (param i32) (result i32) local.get 0)
            )
            (core instance $i (instantiate $m))

            (func (export "u32-to-bool") (param u32) (result bool)
                (canon lift (core func $i "pass"))
            )
            (func (export "bool-to-u32") (param bool) (result u32)
                (canon lift (core func $i "pass"))
            )
        )
    "#;

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;
    let u32_to_bool = instance.get_typed_func::<(u32,), bool, _>(&mut store, "u32-to-bool")?;
    let bool_to_u32 = instance.get_typed_func::<(bool,), u32, _>(&mut store, "bool-to-u32")?;

    assert_eq!(bool_to_u32.call(&mut store, (false,))?, 0);
    assert_eq!(bool_to_u32.call(&mut store, (true,))?, 1);
    assert_eq!(u32_to_bool.call(&mut store, (0,))?, false);
    assert_eq!(u32_to_bool.call(&mut store, (1,))?, true);
    assert_eq!(u32_to_bool.call(&mut store, (2,))?, true);

    Ok(())
}

#[test]
fn chars() -> Result<()> {
    let component = r#"
        (component
            (core module $m
                (func (export "pass") (param i32) (result i32) local.get 0)
            )
            (core instance $i (instantiate $m))

            (func (export "u32-to-char") (param u32) (result char)
                (canon lift (core func $i "pass"))
            )
            (func (export "char-to-u32") (param char) (result u32)
                (canon lift (core func $i "pass"))
            )
        )
    "#;

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;
    let u32_to_char = instance.get_typed_func::<(u32,), char, _>(&mut store, "u32-to-char")?;
    let char_to_u32 = instance.get_typed_func::<(char,), u32, _>(&mut store, "char-to-u32")?;

    let mut roundtrip = |x: char| -> Result<()> {
        assert_eq!(char_to_u32.call(&mut store, (x,))?, x as u32);
        assert_eq!(u32_to_char.call(&mut store, (x as u32,))?, x);
        Ok(())
    };

    roundtrip('x')?;
    roundtrip('a')?;
    roundtrip('\0')?;
    roundtrip('\n')?;
    roundtrip('💝')?;

    let err = u32_to_char.call(&mut store, (0xd800,)).unwrap_err();
    assert!(err.to_string().contains("integer out of range"), "{}", err);
    let err = u32_to_char.call(&mut store, (0xdfff,)).unwrap_err();
    assert!(err.to_string().contains("integer out of range"), "{}", err);
    let err = u32_to_char.call(&mut store, (0x110000,)).unwrap_err();
    assert!(err.to_string().contains("integer out of range"), "{}", err);
    let err = u32_to_char.call(&mut store, (u32::MAX,)).unwrap_err();
    assert!(err.to_string().contains("integer out of range"), "{}", err);

    Ok(())
}

#[test]
fn tuple_result() -> Result<()> {
    let component = r#"
        (component
            (core module $m
                (memory (export "memory") 1)
                (func (export "foo") (param i32 i32 f32 f64) (result i32)
                    (local $base i32)
                    (local.set $base (i32.const 8))
                    (i32.store8 offset=0 (local.get $base) (local.get 0))
                    (i32.store16 offset=2 (local.get $base) (local.get 1))
                    (f32.store offset=4 (local.get $base) (local.get 2))
                    (f64.store offset=8 (local.get $base) (local.get 3))
                    local.get $base
                )

                (func (export "invalid") (result i32)
                    i32.const -8
                )
            )
            (core instance $i (instantiate $m))

            (type $result (tuple s8 u16 float32 float64))
            (func (export "tuple")
                (param s8) (param u16) (param float32) (param float64) (result $result)
                (canon lift (core func $i "foo") (memory $i "memory"))
            )
            (func (export "invalid") (result $result)
                (canon lift (core func $i "invalid") (memory $i "memory"))
            )
        )
    "#;

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;

    let input = (-1, 100, 3.0, 100.0);
    let output = instance
        .get_typed_func::<(i8, u16, f32, f64), (i8, u16, f32, f64), _>(&mut store, "tuple")?
        .call(&mut store, input)?;
    assert_eq!(input, output);

    let invalid_func =
        instance.get_typed_func::<(), (i8, u16, f32, f64), _>(&mut store, "invalid")?;
    let err = invalid_func.call(&mut store, ()).err().unwrap();
    assert!(
        err.to_string().contains("pointer out of bounds of memory"),
        "{}",
        err
    );

    Ok(())
}

#[test]
fn strings() -> Result<()> {
    let component = format!(
        r#"(component
            (core module $m
                (memory (export "memory") 1)
                (func (export "roundtrip") (param i32 i32) (result i32)
                    (local $base i32)
                    (local.set $base
                        (call $realloc
                            (i32.const 0)
                            (i32.const 0)
                            (i32.const 4)
                            (i32.const 8)))
                    (i32.store offset=0
                        (local.get $base)
                        (local.get 0))
                    (i32.store offset=4
                        (local.get $base)
                        (local.get 1))
                    (local.get $base)
                )

                {REALLOC_AND_FREE}
            )
            (core instance $i (instantiate $m))

            (func (export "list8-to-str") (param (list u8)) (result string)
                (canon lift
                    (core func $i "roundtrip")
                    (memory $i "memory")
                    (realloc (func $i "realloc"))
                )
            )
            (func (export "str-to-list8") (param string) (result (list u8))
                (canon lift
                    (core func $i "roundtrip")
                    (memory $i "memory")
                    (realloc (func $i "realloc"))
                )
            )
            (func (export "list16-to-str") (param (list u16)) (result string)
                (canon lift
                    (core func $i "roundtrip")
                    string-encoding=utf16
                    (memory $i "memory")
                    (realloc (func $i "realloc"))
                )
            )
            (func (export "str-to-list16") (param string) (result (list u16))
                (canon lift
                    (core func $i "roundtrip")
                    string-encoding=utf16
                    (memory $i "memory")
                    (realloc (func $i "realloc"))
                )
            )
        )"#
    );

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;
    let list8_to_str =
        instance.get_typed_func::<(&[u8],), WasmStr, _>(&mut store, "list8-to-str")?;
    let str_to_list8 =
        instance.get_typed_func::<(&str,), WasmList<u8>, _>(&mut store, "str-to-list8")?;
    let list16_to_str =
        instance.get_typed_func::<(&[u16],), WasmStr, _>(&mut store, "list16-to-str")?;
    let str_to_list16 =
        instance.get_typed_func::<(&str,), WasmList<u16>, _>(&mut store, "str-to-list16")?;

    let mut roundtrip = |x: &str| -> Result<()> {
        let ret = list8_to_str.call(&mut store, (x.as_bytes(),))?;
        assert_eq!(ret.to_str(&store)?, x);

        let utf16 = x.encode_utf16().collect::<Vec<_>>();
        let ret = list16_to_str.call(&mut store, (&utf16[..],))?;
        assert_eq!(ret.to_str(&store)?, x);

        let ret = str_to_list8.call(&mut store, (x,))?;
        assert_eq!(ret.iter(&store).collect::<Result<Vec<_>>>()?, x.as_bytes());

        let ret = str_to_list16.call(&mut store, (x,))?;
        assert_eq!(ret.iter(&store).collect::<Result<Vec<_>>>()?, utf16,);

        Ok(())
    };

    roundtrip("")?;
    roundtrip("foo")?;
    roundtrip("hello there")?;
    roundtrip("💝")?;
    roundtrip("Löwe 老虎 Léopard")?;

    let ret = list8_to_str.call(&mut store, (b"\xff",))?;
    let err = ret.to_str(&store).unwrap_err();
    assert!(err.to_string().contains("invalid utf-8"), "{}", err);

    let ret = list8_to_str.call(&mut store, (b"hello there \xff invalid",))?;
    let err = ret.to_str(&store).unwrap_err();
    assert!(err.to_string().contains("invalid utf-8"), "{}", err);

    let ret = list16_to_str.call(&mut store, (&[0xd800],))?;
    let err = ret.to_str(&store).unwrap_err();
    assert!(err.to_string().contains("unpaired surrogate"), "{}", err);

    let ret = list16_to_str.call(&mut store, (&[0xdfff],))?;
    let err = ret.to_str(&store).unwrap_err();
    assert!(err.to_string().contains("unpaired surrogate"), "{}", err);

    let ret = list16_to_str.call(&mut store, (&[0xd800, 0xff00],))?;
    let err = ret.to_str(&store).unwrap_err();
    assert!(err.to_string().contains("unpaired surrogate"), "{}", err);

    Ok(())
}

#[test]
fn many_parameters() -> Result<()> {
    let component = format!(
        r#"(component
            (core module $m
                (memory (export "memory") 1)
                (func (export "foo") (param i32) (result i32)
                    (local $base i32)

                    ;; Allocate space for the return
                    (local.set $base
                        (call $realloc
                            (i32.const 0)
                            (i32.const 0)
                            (i32.const 4)
                            (i32.const 12)))

                    ;; Store the pointer/length of the entire linear memory
                    ;; so we have access to everything.
                    (i32.store offset=0
                        (local.get $base)
                        (i32.const 0))
                    (i32.store offset=4
                        (local.get $base)
                        (i32.mul
                            (memory.size)
                            (i32.const 65536)))

                    ;; And also store our pointer parameter
                    (i32.store offset=8
                        (local.get $base)
                        (local.get 0))

                    (local.get $base)
                )

                {REALLOC_AND_FREE}
            )
            (core instance $i (instantiate $m))

            (type $result (tuple (list u8) u32))
            (type $t (func
                (param s8)              ;; offset  0, size 1
                (param u64)             ;; offset  8, size 8
                (param float32)         ;; offset 16, size 4
                (param u8)              ;; offset 20, size 1
                (param unit)            ;; offset 21, size 0
                (param s16)             ;; offset 22, size 2
                (param string)          ;; offset 24, size 8
                (param (list u32))      ;; offset 32, size 8
                (param bool)            ;; offset 40, size 1
                (param bool)            ;; offset 41, size 1
                (param char)            ;; offset 44, size 4
                (param (list bool))     ;; offset 48, size 8
                (param (list char))     ;; offset 56, size 8
                (param (list string))   ;; offset 64, size 8

                (result $result)
            ))
            (func (export "many-param") (type $t)
                (canon lift
                    (core func $i "foo")
                    (memory $i "memory")
                    (realloc (func $i "realloc"))
                )
            )
        )"#
    );

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;
    let func = instance.get_typed_func::<(
        i8,
        u64,
        f32,
        u8,
        (),
        i16,
        &str,
        &[u32],
        bool,
        bool,
        char,
        &[bool],
        &[char],
        &[&str],
    ), (WasmList<u8>, u32), _>(&mut store, "many-param")?;

    let input = (
        -100,
        u64::MAX / 2,
        f32::from_bits(CANON_32BIT_NAN | 1),
        38,
        (),
        18831,
        "this is the first string",
        [1, 2, 3, 4, 5, 6, 7, 8].as_slice(),
        true,
        false,
        '🚩',
        [false, true, false, true, true].as_slice(),
        ['🍌', '🥐', '🍗', '🍙', '🍡'].as_slice(),
        [
            "the quick",
            "brown fox",
            "was too lazy",
            "to jump over the dog",
            "what a demanding dog",
        ]
        .as_slice(),
    );
    let (memory, pointer) = func.call(&mut store, input)?;
    let memory = memory.as_slice(&store);

    let mut actual = &memory[pointer as usize..][..72];
    assert_eq!(i8::from_le_bytes(*actual.take_n::<1>()), input.0);
    actual.skip::<7>();
    assert_eq!(u64::from_le_bytes(*actual.take_n::<8>()), input.1);
    assert_eq!(u32::from_le_bytes(*actual.take_n::<4>()), CANON_32BIT_NAN);
    assert_eq!(u8::from_le_bytes(*actual.take_n::<1>()), input.3);
    actual.skip::<1>();
    assert_eq!(i16::from_le_bytes(*actual.take_n::<2>()), input.5);
    assert_eq!(actual.ptr_len(memory, 1), input.6.as_bytes());
    let mut mem = actual.ptr_len(memory, 4);
    for expected in input.7.iter() {
        assert_eq!(u32::from_le_bytes(*mem.take_n::<4>()), *expected);
    }
    assert!(mem.is_empty());
    assert_eq!(actual.take_n::<1>(), &[input.8 as u8]);
    assert_eq!(actual.take_n::<1>(), &[input.9 as u8]);
    actual.skip::<2>();
    assert_eq!(u32::from_le_bytes(*actual.take_n::<4>()), input.10 as u32);

    // (list bool)
    mem = actual.ptr_len(memory, 1);
    for expected in input.11.iter() {
        assert_eq!(mem.take_n::<1>(), &[*expected as u8]);
    }
    assert!(mem.is_empty());

    // (list char)
    mem = actual.ptr_len(memory, 4);
    for expected in input.12.iter() {
        assert_eq!(u32::from_le_bytes(*mem.take_n::<4>()), *expected as u32);
    }
    assert!(mem.is_empty());

    // (list string)
    mem = actual.ptr_len(memory, 8);
    for expected in input.13.iter() {
        let actual = mem.ptr_len(memory, 1);
        assert_eq!(actual, expected.as_bytes());
    }
    assert!(mem.is_empty());
    assert!(actual.is_empty());

    Ok(())
}

#[test]
fn some_traps() -> Result<()> {
    let middle_of_memory = (i32::MAX / 2) & (!0xff);
    let component = format!(
        r#"(component
            (core module $m
                (memory (export "memory") 1)
                (func (export "take-many") (param i32))
                (func (export "take-list") (param i32 i32))

                (func (export "realloc") (param i32 i32 i32 i32) (result i32)
                    unreachable)
            )
            (core instance $i (instantiate $m))

            (func (export "take-list-unreachable") (param (list u8))
                (canon lift (core func $i "take-list") (memory $i "memory") (realloc (func $i "realloc")))
            )
            (func (export "take-string-unreachable") (param string)
                (canon lift (core func $i "take-list") (memory $i "memory") (realloc (func $i "realloc")))
            )

            (type $t (func
                (param string)
                (param string)
                (param string)
                (param string)
                (param string)
                (param string)
                (param string)
                (param string)
                (param string)
                (param string)
            ))
            (func (export "take-many-unreachable") (type $t)
                (canon lift (core func $i "take-many") (memory $i "memory") (realloc (func $i "realloc")))
            )

            (core module $m2
                (memory (export "memory") 1)
                (func (export "take-many") (param i32))
                (func (export "take-list") (param i32 i32))

                (func (export "realloc") (param i32 i32 i32 i32) (result i32)
                    i32.const {middle_of_memory})
            )
            (core instance $i2 (instantiate $m2))

            (func (export "take-list-base-oob") (param (list u8))
                (canon lift (core func $i2 "take-list") (memory $i2 "memory") (realloc (func $i2 "realloc")))
            )
            (func (export "take-string-base-oob") (param string)
                (canon lift (core func $i2 "take-list") (memory $i2 "memory") (realloc (func $i2 "realloc")))
            )
            (func (export "take-many-base-oob") (type $t)
                (canon lift (core func $i2 "take-many") (memory $i2 "memory") (realloc (func $i2 "realloc")))
            )

            (core module $m3
                (memory (export "memory") 1)
                (func (export "take-many") (param i32))
                (func (export "take-list") (param i32 i32))

                (func (export "realloc") (param i32 i32 i32 i32) (result i32)
                    i32.const 65532)
            )
            (core instance $i3 (instantiate $m3))

            (func (export "take-list-end-oob") (param (list u8))
                (canon lift (core func $i3 "take-list") (memory $i3 "memory") (realloc (func $i3 "realloc")))
            )
            (func (export "take-string-end-oob") (param string)
                (canon lift (core func $i3 "take-list") (memory $i3 "memory") (realloc (func $i3 "realloc")))
            )
            (func (export "take-many-end-oob") (type $t)
                (canon lift (core func $i3 "take-many") (memory $i3 "memory") (realloc (func $i3 "realloc")))
            )

            (core module $m4
                (memory (export "memory") 1)
                (func (export "take-many") (param i32))

                (global $cnt (mut i32) (i32.const 0))
                (func (export "realloc") (param i32 i32 i32 i32) (result i32)
                    global.get $cnt
                    if (result i32)
                        i32.const 100000
                    else
                        i32.const 1
                        global.set $cnt
                        i32.const 0
                    end
                )
            )
            (core instance $i4 (instantiate $m4))

            (func (export "take-many-second-oob") (type $t)
                (canon lift (core func $i4 "take-many") (memory $i4 "memory") (realloc (func $i4 "realloc")))
            )
        )"#
    );

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;

    // This should fail when calling the allocator function for the argument
    let err = instance
        .get_typed_func::<(&[u8],), (), _>(&mut store, "take-list-unreachable")?
        .call(&mut store, (&[],))
        .unwrap_err()
        .downcast::<Trap>()?;
    assert_eq!(err.trap_code(), Some(TrapCode::UnreachableCodeReached));

    // This should fail when calling the allocator function for the argument
    let err = instance
        .get_typed_func::<(&str,), (), _>(&mut store, "take-string-unreachable")?
        .call(&mut store, ("",))
        .unwrap_err()
        .downcast::<Trap>()?;
    assert_eq!(err.trap_code(), Some(TrapCode::UnreachableCodeReached));

    // This should fail when calling the allocator function for the space
    // to store the arguments (before arguments are even lowered)
    let err = instance
        .get_typed_func::<(&str, &str, &str, &str, &str, &str, &str, &str, &str, &str), (), _>(
            &mut store,
            "take-many-unreachable",
        )?
        .call(&mut store, ("", "", "", "", "", "", "", "", "", ""))
        .unwrap_err()
        .downcast::<Trap>()?;
    assert_eq!(err.trap_code(), Some(TrapCode::UnreachableCodeReached));

    // Assert that when the base pointer returned by malloc is out of bounds
    // that errors are reported as such. Both empty and lists with contents
    // should all be invalid here.
    //
    // FIXME(WebAssembly/component-model#32) confirm the semantics here are
    // what's desired.
    #[track_caller]
    fn assert_oob(err: &anyhow::Error) {
        assert!(
            err.to_string()
                .contains("realloc return: beyond end of memory"),
            "{:?}",
            err,
        );
    }
    let err = instance
        .get_typed_func::<(&[u8],), (), _>(&mut store, "take-list-base-oob")?
        .call(&mut store, (&[],))
        .unwrap_err();
    assert_oob(&err);
    let err = instance
        .get_typed_func::<(&[u8],), (), _>(&mut store, "take-list-base-oob")?
        .call(&mut store, (&[1],))
        .unwrap_err();
    assert_oob(&err);
    let err = instance
        .get_typed_func::<(&str,), (), _>(&mut store, "take-string-base-oob")?
        .call(&mut store, ("",))
        .unwrap_err();
    assert_oob(&err);
    let err = instance
        .get_typed_func::<(&str,), (), _>(&mut store, "take-string-base-oob")?
        .call(&mut store, ("x",))
        .unwrap_err();
    assert_oob(&err);
    let err = instance
        .get_typed_func::<(&str, &str, &str, &str, &str, &str, &str, &str, &str, &str), (), _>(
            &mut store,
            "take-many-base-oob",
        )?
        .call(&mut store, ("", "", "", "", "", "", "", "", "", ""))
        .unwrap_err();
    assert_oob(&err);

    // Test here that when the returned pointer from malloc is one byte from the
    // end of memory that empty things are fine, but larger things are not.

    instance
        .get_typed_func::<(&[u8],), (), _>(&mut store, "take-list-end-oob")?
        .call(&mut store, (&[],))?;
    instance
        .get_typed_func::<(&[u8],), (), _>(&mut store, "take-list-end-oob")?
        .call(&mut store, (&[1, 2, 3, 4],))?;
    let err = instance
        .get_typed_func::<(&[u8],), (), _>(&mut store, "take-list-end-oob")?
        .call(&mut store, (&[1, 2, 3, 4, 5],))
        .unwrap_err();
    assert_oob(&err);
    instance
        .get_typed_func::<(&str,), (), _>(&mut store, "take-string-end-oob")?
        .call(&mut store, ("",))?;
    instance
        .get_typed_func::<(&str,), (), _>(&mut store, "take-string-end-oob")?
        .call(&mut store, ("abcd",))?;
    let err = instance
        .get_typed_func::<(&str,), (), _>(&mut store, "take-string-end-oob")?
        .call(&mut store, ("abcde",))
        .unwrap_err();
    assert_oob(&err);
    let err = instance
        .get_typed_func::<(&str, &str, &str, &str, &str, &str, &str, &str, &str, &str), (), _>(
            &mut store,
            "take-many-end-oob",
        )?
        .call(&mut store, ("", "", "", "", "", "", "", "", "", ""))
        .unwrap_err();
    assert_oob(&err);

    // For this function the first allocation, the space to store all the
    // arguments, is in-bounds but then all further allocations, such as for
    // each individual string, are all out of bounds.
    let err = instance
        .get_typed_func::<(&str, &str, &str, &str, &str, &str, &str, &str, &str, &str), (), _>(
            &mut store,
            "take-many-second-oob",
        )?
        .call(&mut store, ("", "", "", "", "", "", "", "", "", ""))
        .unwrap_err();
    assert_oob(&err);
    Ok(())
}

#[test]
fn char_bool_memory() -> Result<()> {
    let component = format!(
        r#"(component
            (core module $m
                (memory (export "memory") 1)
                (func (export "ret-tuple") (param i32 i32) (result i32)
                    (local $base i32)

                    ;; Allocate space for the return
                    (local.set $base
                        (call $realloc
                            (i32.const 0)
                            (i32.const 0)
                            (i32.const 4)
                            (i32.const 8)))

                    ;; store the boolean
                    (i32.store offset=0
                        (local.get $base)
                        (local.get 0))

                    ;; store the char
                    (i32.store offset=4
                        (local.get $base)
                        (local.get 1))

                    (local.get $base)
                )

                {REALLOC_AND_FREE}
            )
            (core instance $i (instantiate $m))

            (func (export "ret-tuple") (param u32) (param u32) (result (tuple bool char))
                (canon lift (core func $i "ret-tuple")
                    (memory $i "memory")
                    (realloc (func $i "realloc")))
            )
        )"#
    );

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;
    let func = instance.get_typed_func::<(u32, u32), (bool, char), _>(&mut store, "ret-tuple")?;

    let ret = func.call(&mut store, (0, 'a' as u32))?;
    assert_eq!(ret, (false, 'a'));

    let ret = func.call(&mut store, (1, '🍰' as u32))?;
    assert_eq!(ret, (true, '🍰'));

    let ret = func.call(&mut store, (2, 'a' as u32))?;
    assert_eq!(ret, (true, 'a'));

    assert!(func.call(&mut store, (0, 0xd800)).is_err());

    Ok(())
}

#[test]
fn string_list_oob() -> Result<()> {
    let component = format!(
        r#"(component
            (core module $m
                (memory (export "memory") 1)
                (func (export "ret-list") (result i32)
                    (local $base i32)

                    ;; Allocate space for the return
                    (local.set $base
                        (call $realloc
                            (i32.const 0)
                            (i32.const 0)
                            (i32.const 4)
                            (i32.const 8)))

                    (i32.store offset=0
                        (local.get $base)
                        (i32.const 100000))
                    (i32.store offset=4
                        (local.get $base)
                        (i32.const 1))

                    (local.get $base)
                )

                {REALLOC_AND_FREE}
            )
            (core instance $i (instantiate $m))

            (func (export "ret-list-u8") (result (list u8))
                (canon lift (core func $i "ret-list")
                    (memory $i "memory")
                    (realloc (func $i "realloc"))
                )
            )
            (func (export "ret-string") (result string)
                (canon lift (core func $i "ret-list")
                    (memory $i "memory")
                    (realloc (func $i "realloc"))
                )
            )
        )"#
    );

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;
    let ret_list_u8 = instance.get_typed_func::<(), WasmList<u8>, _>(&mut store, "ret-list-u8")?;
    let ret_string = instance.get_typed_func::<(), WasmStr, _>(&mut store, "ret-string")?;

    let err = ret_list_u8.call(&mut store, ()).err().unwrap();
    assert!(err.to_string().contains("out of bounds"), "{}", err);

    let err = ret_string.call(&mut store, ()).err().unwrap();
    assert!(err.to_string().contains("out of bounds"), "{}", err);

    Ok(())
}

#[test]
fn tuples() -> Result<()> {
    let component = format!(
        r#"(component
            (core module $m
                (memory (export "memory") 1)
                (func (export "foo")
                    (param i32 f64 i32)
                    (result i32)

                    local.get 0
                    i32.const 0
                    i32.ne
                    if unreachable end

                    local.get 1
                    f64.const 1
                    f64.ne
                    if unreachable end

                    local.get 2
                    i32.const 2
                    i32.ne
                    if unreachable end

                    i32.const 3
                )
            )
            (core instance $i (instantiate $m))

            (func (export "foo")
                (param (tuple s32 float64))
                (param (tuple s8))
                (result (tuple u16))
                (canon lift (core func $i "foo"))
            )
        )"#
    );

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;
    let foo = instance.get_typed_func::<((i32, f64), (i8,)), (u16,), _>(&mut store, "foo")?;
    assert_eq!(foo.call(&mut store, ((0, 1.0), (2,)))?, (3,));

    Ok(())
}

#[test]
fn option() -> Result<()> {
    let component = format!(
        r#"(component
            (core module $m
                (memory (export "memory") 1)
                (func (export "pass0") (param i32) (result i32)
                    local.get 0
                )
                (func (export "pass1") (param i32 i32) (result i32)
                    (local $base i32)
                    (local.set $base
                        (call $realloc
                            (i32.const 0)
                            (i32.const 0)
                            (i32.const 4)
                            (i32.const 8)))

                    (i32.store offset=0
                        (local.get $base)
                        (local.get 0))
                    (i32.store offset=4
                        (local.get $base)
                        (local.get 1))

                    (local.get $base)
                )
                (func (export "pass2") (param i32 i32 i32) (result i32)
                    (local $base i32)
                    (local.set $base
                        (call $realloc
                            (i32.const 0)
                            (i32.const 0)
                            (i32.const 4)
                            (i32.const 12)))

                    (i32.store offset=0
                        (local.get $base)
                        (local.get 0))
                    (i32.store offset=4
                        (local.get $base)
                        (local.get 1))
                    (i32.store offset=8
                        (local.get $base)
                        (local.get 2))

                    (local.get $base)
                )

                {REALLOC_AND_FREE}
            )
            (core instance $i (instantiate $m))

            (func (export "option-unit-to-u32") (param (option unit)) (result u32)
                (canon lift (core func $i "pass0"))
            )
            (func (export "option-u8-to-tuple") (param (option u8)) (result (tuple u32 u32))
                (canon lift (core func $i "pass1") (memory $i "memory"))
            )
            (func (export "option-u32-to-tuple") (param (option u32)) (result (tuple u32 u32))
                (canon lift (core func $i "pass1") (memory $i "memory"))
            )
            (func (export "option-string-to-tuple") (param (option string)) (result (tuple u32 string))
                (canon lift
                    (core func $i "pass2")
                    (memory $i "memory")
                    (realloc (func $i "realloc"))
                )
            )
            (func (export "to-option-unit") (param u32) (result (option unit))
                (canon lift (core func $i "pass0"))
            )
            (func (export "to-option-u8") (param u32) (param u32) (result (option u8))
                (canon lift (core func $i "pass1") (memory $i "memory"))
            )
            (func (export "to-option-u32") (param u32) (param u32) (result (option u32))
                (canon lift
                    (core func $i "pass1")
                    (memory $i "memory")
                )
            )
            (func (export "to-option-string") (param u32) (param string) (result (option string))
                (canon lift
                    (core func $i "pass2")
                    (memory $i "memory")
                    (realloc (func $i "realloc"))
                )
            )
        )"#
    );

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;
    let option_unit_to_u32 =
        instance.get_typed_func::<(Option<()>,), u32, _>(&mut store, "option-unit-to-u32")?;
    assert_eq!(option_unit_to_u32.call(&mut store, (None,))?, 0);
    assert_eq!(option_unit_to_u32.call(&mut store, (Some(()),))?, 1);

    let option_u8_to_tuple = instance
        .get_typed_func::<(Option<u8>,), (u32, u32), _>(&mut store, "option-u8-to-tuple")?;
    assert_eq!(option_u8_to_tuple.call(&mut store, (None,))?, (0, 0));
    assert_eq!(option_u8_to_tuple.call(&mut store, (Some(0),))?, (1, 0));
    assert_eq!(option_u8_to_tuple.call(&mut store, (Some(100),))?, (1, 100));

    let option_u32_to_tuple = instance
        .get_typed_func::<(Option<u32>,), (u32, u32), _>(&mut store, "option-u32-to-tuple")?;
    assert_eq!(option_u32_to_tuple.call(&mut store, (None,))?, (0, 0));
    assert_eq!(option_u32_to_tuple.call(&mut store, (Some(0),))?, (1, 0));
    assert_eq!(
        option_u32_to_tuple.call(&mut store, (Some(100),))?,
        (1, 100)
    );

    let option_string_to_tuple = instance.get_typed_func::<(Option<&str>,), (u32, WasmStr), _>(
        &mut store,
        "option-string-to-tuple",
    )?;
    let (a, b) = option_string_to_tuple.call(&mut store, (None,))?;
    assert_eq!(a, 0);
    assert_eq!(b.to_str(&store)?, "");
    let (a, b) = option_string_to_tuple.call(&mut store, (Some(""),))?;
    assert_eq!(a, 1);
    assert_eq!(b.to_str(&store)?, "");
    let (a, b) = option_string_to_tuple.call(&mut store, (Some("hello"),))?;
    assert_eq!(a, 1);
    assert_eq!(b.to_str(&store)?, "hello");

    let to_option_unit =
        instance.get_typed_func::<(u32,), Option<()>, _>(&mut store, "to-option-unit")?;
    assert_eq!(to_option_unit.call(&mut store, (0,))?, None);
    assert_eq!(to_option_unit.call(&mut store, (1,))?, Some(()));
    let err = to_option_unit.call(&mut store, (2,)).unwrap_err();
    assert!(err.to_string().contains("invalid option"), "{}", err);

    let to_option_u8 =
        instance.get_typed_func::<(u32, u32), Option<u8>, _>(&mut store, "to-option-u8")?;
    assert_eq!(to_option_u8.call(&mut store, (0x00_00, 0))?, None);
    assert_eq!(to_option_u8.call(&mut store, (0x00_01, 0))?, Some(0));
    assert_eq!(to_option_u8.call(&mut store, (0xfd_01, 0))?, Some(0xfd));
    assert!(to_option_u8.call(&mut store, (0x00_02, 0)).is_err());

    let to_option_u32 =
        instance.get_typed_func::<(u32, u32), Option<u32>, _>(&mut store, "to-option-u32")?;
    assert_eq!(to_option_u32.call(&mut store, (0, 0))?, None);
    assert_eq!(to_option_u32.call(&mut store, (1, 0))?, Some(0));
    assert_eq!(
        to_option_u32.call(&mut store, (1, 0x1234fead))?,
        Some(0x1234fead)
    );
    assert!(to_option_u32.call(&mut store, (2, 0)).is_err());

    let to_option_string = instance
        .get_typed_func::<(u32, &str), Option<WasmStr>, _>(&mut store, "to-option-string")?;
    let ret = to_option_string.call(&mut store, (0, ""))?;
    assert!(ret.is_none());
    let ret = to_option_string.call(&mut store, (1, ""))?;
    assert_eq!(ret.unwrap().to_str(&store)?, "");
    let ret = to_option_string.call(&mut store, (1, "cheesecake"))?;
    assert_eq!(ret.unwrap().to_str(&store)?, "cheesecake");
    assert!(to_option_string.call(&mut store, (2, "")).is_err());

    Ok(())
}

#[test]
fn expected() -> Result<()> {
    let component = format!(
        r#"(component
            (core module $m
                (memory (export "memory") 1)
                (func (export "pass0") (param i32) (result i32)
                    local.get 0
                )
                (func (export "pass1") (param i32 i32) (result i32)
                    (local $base i32)
                    (local.set $base
                        (call $realloc
                            (i32.const 0)
                            (i32.const 0)
                            (i32.const 4)
                            (i32.const 8)))

                    (i32.store offset=0
                        (local.get $base)
                        (local.get 0))
                    (i32.store offset=4
                        (local.get $base)
                        (local.get 1))

                    (local.get $base)
                )
                (func (export "pass2") (param i32 i32 i32) (result i32)
                    (local $base i32)
                    (local.set $base
                        (call $realloc
                            (i32.const 0)
                            (i32.const 0)
                            (i32.const 4)
                            (i32.const 12)))

                    (i32.store offset=0
                        (local.get $base)
                        (local.get 0))
                    (i32.store offset=4
                        (local.get $base)
                        (local.get 1))
                    (i32.store offset=8
                        (local.get $base)
                        (local.get 2))

                    (local.get $base)
                )

                {REALLOC_AND_FREE}
            )
            (core instance $i (instantiate $m))

            (func (export "take-expected-unit") (param (expected unit unit)) (result u32)
                (canon lift (core func $i "pass0"))
            )
            (func (export "take-expected-u8-f32") (param (expected u8 float32)) (result (tuple u32 u32))
                (canon lift (core func $i "pass1") (memory $i "memory"))
            )
            (type $list (list u8))
            (func (export "take-expected-string") (param (expected string $list)) (result (tuple u32 string))
                (canon lift
                    (core func $i "pass2")
                    (memory $i "memory")
                    (realloc (func $i "realloc"))
                )
            )
            (func (export "to-expected-unit") (param u32) (result (expected unit unit))
                (canon lift (core func $i "pass0"))
            )
            (func (export "to-expected-s16-f32") (param u32) (param u32) (result (expected s16 float32))
                (canon lift
                    (core func $i "pass1")
                    (memory $i "memory")
                    (realloc (func $i "realloc"))
                )
            )
        )"#
    );

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;
    let take_expected_unit =
        instance.get_typed_func::<(Result<(), ()>,), u32, _>(&mut store, "take-expected-unit")?;
    assert_eq!(take_expected_unit.call(&mut store, (Ok(()),))?, 0);
    assert_eq!(take_expected_unit.call(&mut store, (Err(()),))?, 1);

    let take_expected_u8_f32 = instance
        .get_typed_func::<(Result<u8, f32>,), (u32, u32), _>(&mut store, "take-expected-u8-f32")?;
    assert_eq!(take_expected_u8_f32.call(&mut store, (Ok(1),))?, (0, 1));
    assert_eq!(
        take_expected_u8_f32.call(&mut store, (Err(2.0),))?,
        (1, 2.0f32.to_bits())
    );

    let take_expected_string = instance
        .get_typed_func::<(Result<&str, &[u8]>,), (u32, WasmStr), _>(
            &mut store,
            "take-expected-string",
        )?;
    let (a, b) = take_expected_string.call(&mut store, (Ok("hello"),))?;
    assert_eq!(a, 0);
    assert_eq!(b.to_str(&store)?, "hello");
    let (a, b) = take_expected_string.call(&mut store, (Err(b"goodbye"),))?;
    assert_eq!(a, 1);
    assert_eq!(b.to_str(&store)?, "goodbye");

    let to_expected_unit =
        instance.get_typed_func::<(u32,), Result<(), ()>, _>(&mut store, "to-expected-unit")?;
    assert_eq!(to_expected_unit.call(&mut store, (0,))?, Ok(()));
    assert_eq!(to_expected_unit.call(&mut store, (1,))?, Err(()));
    let err = to_expected_unit.call(&mut store, (2,)).unwrap_err();
    assert!(err.to_string().contains("invalid expected"), "{}", err);

    let to_expected_s16_f32 = instance
        .get_typed_func::<(u32, u32), Result<i16, f32>, _>(&mut store, "to-expected-s16-f32")?;
    assert_eq!(to_expected_s16_f32.call(&mut store, (0, 0))?, Ok(0));
    assert_eq!(to_expected_s16_f32.call(&mut store, (0, 100))?, Ok(100));
    assert_eq!(
        to_expected_s16_f32.call(&mut store, (1, 1.0f32.to_bits()))?,
        Err(1.0)
    );
    let ret = to_expected_s16_f32.call(&mut store, (1, CANON_32BIT_NAN | 1))?;
    assert_eq!(ret.unwrap_err().to_bits(), CANON_32BIT_NAN);
    assert!(to_expected_s16_f32.call(&mut store, (2, 0)).is_err());

    Ok(())
}

#[test]
fn fancy_list() -> Result<()> {
    let component = format!(
        r#"(component
            (core module $m
                (memory (export "memory") 1)
                (func (export "take") (param i32 i32) (result i32)
                    (local $base i32)
                    (local.set $base
                        (call $realloc
                            (i32.const 0)
                            (i32.const 0)
                            (i32.const 4)
                            (i32.const 16)))

                    (i32.store offset=0
                        (local.get $base)
                        (local.get 0))
                    (i32.store offset=4
                        (local.get $base)
                        (local.get 1))
                    (i32.store offset=8
                        (local.get $base)
                        (i32.const 0))
                    (i32.store offset=12
                        (local.get $base)
                        (i32.mul
                            (memory.size)
                            (i32.const 65536)))

                    (local.get $base)
                )

                {REALLOC_AND_FREE}
            )
            (core instance $i (instantiate $m))

            (type $a (option u8))
            (type $b (expected unit string))
            (type $input (list (tuple $a $b)))
            (type $output (tuple u32 u32 (list u8)))
            (func (export "take") (param $input) (result $output)
                (canon lift
                    (core func $i "take")
                    (memory $i "memory")
                    (realloc (func $i "realloc"))
                )
            )
        )"#
    );

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;

    let func = instance
        .get_typed_func::<(&[(Option<u8>, Result<(), &str>)],), (u32, u32, WasmList<u8>), _>(
            &mut store, "take",
        )?;

    let input = [
        (None, Ok(())),
        (Some(2), Err("hello there")),
        (Some(200), Err("general kenobi")),
    ];
    let (ptr, len, list) = func.call(&mut store, (&input,))?;
    let memory = list.as_slice(&store);
    let ptr = usize::try_from(ptr).unwrap();
    let len = usize::try_from(len).unwrap();
    let mut array = &memory[ptr..][..len * 16];

    for (a, b) in input.iter() {
        match a {
            Some(val) => {
                assert_eq!(*array.take_n::<2>(), [1, *val]);
            }
            None => {
                assert_eq!(*array.take_n::<1>(), [0]);
                array.skip::<1>();
            }
        }
        array.skip::<2>();
        match b {
            Ok(()) => {
                assert_eq!(*array.take_n::<1>(), [0]);
                array.skip::<11>();
            }
            Err(s) => {
                assert_eq!(*array.take_n::<1>(), [1]);
                array.skip::<3>();
                assert_eq!(array.ptr_len(memory, 1), s.as_bytes());
            }
        }
    }
    assert!(array.is_empty());

    Ok(())
}

trait SliceExt<'a> {
    fn take_n<const N: usize>(&mut self) -> &'a [u8; N];

    fn skip<const N: usize>(&mut self) {
        self.take_n::<N>();
    }

    fn ptr_len<'b>(&mut self, all_memory: &'b [u8], size: usize) -> &'b [u8] {
        let ptr = u32::from_le_bytes(*self.take_n::<4>());
        let len = u32::from_le_bytes(*self.take_n::<4>());
        let ptr = usize::try_from(ptr).unwrap();
        let len = usize::try_from(len).unwrap();
        &all_memory[ptr..][..len * size]
    }
}

impl<'a> SliceExt<'a> for &'a [u8] {
    fn take_n<const N: usize>(&mut self) -> &'a [u8; N] {
        let (a, b) = self.split_at(N);
        *self = b;
        a.try_into().unwrap()
    }
}

#[test]
fn invalid_alignment() -> Result<()> {
    let component = format!(
        r#"(component
            (core module $m
                (memory (export "memory") 1)
                (func (export "realloc") (param i32 i32 i32 i32) (result i32)
                    i32.const 1)

                (func (export "take-i32") (param i32))
                (func (export "ret-1") (result i32) i32.const 1)
                (func (export "ret-unaligned-list") (result i32)
                    (i32.store offset=0 (i32.const 8) (i32.const 1))
                    (i32.store offset=4 (i32.const 8) (i32.const 1))
                    i32.const 8)
            )
            (core instance $i (instantiate $m))

            (func (export "many-params")
                (param string) (param string) (param string) (param string)
                (param string) (param string) (param string) (param string)
                (param string) (param string) (param string) (param string)
                (canon lift
                    (core func $i "take-i32")
                    (memory $i "memory")
                    (realloc (func $i "realloc"))
                )
            )
            (func (export "string-ret") (result string)
                (canon lift
                    (core func $i "ret-1")
                    (memory $i "memory")
                    (realloc (func $i "realloc"))
                )
            )
            (func (export "list-u32-ret") (result (list u32))
                (canon lift
                    (core func $i "ret-unaligned-list")
                    (memory $i "memory")
                    (realloc (func $i "realloc"))
                )
            )
        )"#
    );

    let engine = super::engine();
    let component = Component::new(&engine, component)?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate(&mut store, &component)?;

    let err = instance
        .get_typed_func::<(
            &str,
            &str,
            &str,
            &str,
            &str,
            &str,
            &str,
            &str,
            &str,
            &str,
            &str,
            &str,
        ), (), _>(&mut store, "many-params")?
        .call(&mut store, ("", "", "", "", "", "", "", "", "", "", "", ""))
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("realloc return: result not aligned"),
        "{}",
        err
    );

    let err = instance
        .get_typed_func::<(), WasmStr, _>(&mut store, "string-ret")?
        .call(&mut store, ())
        .err()
        .unwrap();
    assert!(
        err.to_string().contains("return pointer not aligned"),
        "{}",
        err
    );

    let err = instance
        .get_typed_func::<(), WasmList<u32>, _>(&mut store, "list-u32-ret")?
        .call(&mut store, ())
        .err()
        .unwrap();
    assert!(
        err.to_string().contains("list pointer is not aligned"),
        "{}",
        err
    );

    Ok(())
}
