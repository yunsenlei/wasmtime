use anyhow::Result;
use wasmtime::*;
use std::str;

fn main() -> Result<()> {
    let engine = Engine::default();
    let mut store = Store::new(&engine, ());
    store.set_multi_process();
    
    // extra steps to associate description data with function and register storage
    let mut prty_collection = FuncPRTypeCollection::new();
    let pr_type = [
        ParamRetType::from(BufferType::wasm_out(Idx::param_idx(0), Idx::param_idx(1))), 
        ParamRetType::from(BufferType::wasm_in(Idx::param_idx(2), Idx::param_idx(3))),
        ParamRetType::from(UnType::new(Idx::RetIdx(0))),
        ];
    let index = store.reserve_func_storage("log_str"); 
    prty_collection.insert(index, "log_str",pr_type)?;
    //------------------------------------------------------------------
    
    let module = Module::from_file_with_pr_types(&engine, "target/debug/examples/log-str.wat", &prty_collection)?;
    let log_str = Func::wrap(&mut store,
        |mut caller: Caller<'_, ()>, arg0: i32, arg1: i32, arg2: i32, arg3: i32| -> i32 {    
        println!("enter log_str");

        let mem = caller.pull_current_func_data_mut();
        let data = mem.get_data_from_ptr_and_len(arg0, arg1);
        let string = match data {
            Some(data) => match str::from_utf8(data) {
                Ok(s) => s,
                Err(_) => return 1,
            },
            None => return 1,
        };
        println!("{}", string);

        mem.set_data_for_ptr_and_len(arg2, arg3, "Some data back".as_bytes());
        println!("exit log_str");
        0
    });
    let instance = Instance::new_for_multi_process(&mut store, &module, 
        &[("log_str", log_str.into())])?;
    let foo = instance.get_func(& mut store, "foo").unwrap();
    foo.spawn_and_call(&instance, &mut store)?;
    Ok(())
}
