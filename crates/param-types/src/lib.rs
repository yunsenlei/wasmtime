use std::fmt::Debug;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::{Entry, HashMap};
use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum Idx{
    #[serde(rename = "param_idx")]
    ParamIdx(usize),
    #[serde(rename = "ret_idx")]
    RetIdx(usize),
}

impl Idx {
    pub fn param_idx(u: usize) -> Self {
        Idx::ParamIdx(u)
    }

    pub fn ret_idx(u: usize) -> Self {
        Idx::RetIdx(u)
    }
}

impl Into<usize> for Idx{
    fn into(self) -> usize {
        match self {
            Idx::ParamIdx(u) => u,
            Idx::RetIdx(u) => u,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum BufferType{
    /// WasmIn
    #[serde(rename = "wasm_in_buf")]
    WasmIn{
        /// ptr 
        ptr_idx: Idx,
        /// len
        len_idx: Idx,
    }, 
    /// WasmOut 
    #[serde(rename = "wasm_out_buf")]
    WasmOut{
        /// ptr
        ptr_idx: Idx, 
        /// len
        len_idx: Idx
    }
}

impl BufferType{
    /// create wasm's inbuffer description from ptr and len index
    pub fn wasm_in(ptr_idx: Idx, len_idx: Idx) -> Self{
        BufferType::WasmIn { ptr_idx, len_idx} 
    }

    /// /// create wasm's outbuffer description from ptr and len index
    pub fn wasm_out(ptr_idx: Idx, len_idx: Idx) -> Self {
        BufferType::WasmOut { ptr_idx, len_idx}
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename = "unassociated")]
pub struct UnType(pub Idx);

impl UnType {
    pub fn new(i: Idx) -> Self{
        UnType(i)
    }
}

impl From<Idx> for UnType{
    fn from(i: Idx) -> Self {
        UnType(i)
    }
}

impl Into<Idx> for UnType{
    fn into(self) -> Idx {
        match self {
            UnType(i) => i
        }
    }
}

/// ParamRetType
#[derive(Debug, Clone, Copy)]
pub enum ParamRetType{
    /// The Buffer type
    Buffer(BufferType),
    Un(UnType)
}

impl From<BufferType> for ParamRetType {
    fn from(b: BufferType) -> Self {
        ParamRetType::Buffer(b)
    }
}

impl From<UnType> for ParamRetType{
    fn from(u: UnType) -> Self {
        ParamRetType::Un(u)
    }
}

/// Used to index the storage for the function's parameter and underlying data
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq)]
pub struct FuncDataIndex(usize);

impl FuncDataIndex{
    pub fn reserved() -> Self {
        FuncDataIndex(usize::MAX)
    }
}

impl From<usize> for FuncDataIndex{
    fn from(u: usize) -> Self {
        FuncDataIndex(u)
    }
}

impl Into<usize> for FuncDataIndex{
    fn into(self) -> usize {
        match self {
            FuncDataIndex(u) => u
        }
    }
}

/// FuncParamRetTypes
///
/// A descriptor of a function's parameter and return value type. 
/// Unlike the WasmFuncType, here the type suggests the interpretation of values in terms of memory
/// We regroup pairs of parameters and return into Buffers, or leave them as Unassociated
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename = "func_param_ret_types")]
pub struct FuncParamRetTypes{
    /// Buffer type parameter (index)
    #[serde(rename = "buffer_type_params")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    bty: Vec<BufferType>,

    // /// Buffer type results (index)
    // #[serde(rename = "buffer_type_returns")]
    // #[serde(skip_serializing_if = "Vec::is_empty")]
    // r_bty: Vec<BufferType<P, L>>,

    /// Unassociated type parameter (index)
    #[serde(rename = "unassociated_params")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    uty: Vec<UnType>,

    // /// Unassociated type results (index)
    // #[serde(rename = "unassociated_returns")]
    // #[serde(skip_serializing_if = "Vec::is_empty")]
    // r_uty: Vec<UnRetIdx>,

    /// The key (index) to locate the underlying storage 
    #[serde(skip)]
    idx: FuncDataIndex,

    /// The name of the function
    func_name: String,
}

impl FuncParamRetTypes {

    pub fn new(ty_list: impl IntoIterator<Item = ParamRetType>, func_name: &str) -> Self{
        let mut bty = Vec::new();
        let mut uty = Vec::new();

        for ty in ty_list {
            match ty {
                ParamRetType::Buffer(b) => {
                    bty.push(b)
                },
                ParamRetType::Un(u) => {
                    uty.push(u)
                },
            }
        }

        FuncParamRetTypes {bty, uty, idx: FuncDataIndex::reserved(), func_name: func_name.to_string()}
    }

    pub fn new_with_reserverd_storage(ty_list: impl IntoIterator<Item = ParamRetType>, index: usize, func_name: &str) -> Self{
        let mut bty = Vec::new();
        let mut uty = Vec::new();
        
        for ty in ty_list {
            match ty {
                ParamRetType::Buffer(b) => {
                    bty.push(b)
                },
                ParamRetType::Un(u) => {
                    uty.push(u)
                },
            }
        }

        FuncParamRetTypes {bty, uty, idx: FuncDataIndex::from(index), func_name: func_name.to_string()}
    }

    /// Return buffer type parameters
    pub fn buffer_vec(&self)-> &Vec<BufferType>{
        &self.bty
    }


    /// Return the unassociated type parameters
    pub fn un_vec(&self) -> &Vec<UnType>{
        &self.uty
    }

    /// Return the index of the storage for data
    pub fn index(&self) -> Option<usize> {
        if self.idx == FuncDataIndex::reserved(){
            None
        } else {
            Some(self.idx.into())
        }
        
    }

    pub fn func_name(&self) -> &str {
        self.func_name.as_str()
    }

    /// dislay the parameter type
    pub fn display_param_types(&self) {
        if self.bty.len() > 0 {
            println!("Parameter of BufferTypes:");
            for b in &self.bty {
                println!("\t{:?}", b);
            }
        }
        if self.uty.len() > 0 {
            println!("Unassociated Parameters:");
            for u in &self.uty {
                println!("\t{:?}", u);
            }
        }
    }

    pub fn serialize_to_json(&self){
        let ser_json = serde_json::to_string(self).unwrap();
        println!("serialized = {}", ser_json);
    }

}

/// FuncPRTypeCollection
///
/// A collection of parameter types, indexed by name
#[derive(Debug, Clone)]
pub struct FuncPRTypeCollection(HashMap<String, FuncParamRetTypes>);

impl FuncPRTypeCollection{
    /// Create the FuncParamTypeCollection
    pub fn new() -> Self{
        FuncPRTypeCollection(HashMap::new())
    }

    /// Insert an new record
    pub fn insert(&mut self, index: usize, key: &str, ty_list: impl IntoIterator<Item = ParamRetType>) -> Result<()>{
        match self.0.entry(key.to_string()){
            Entry::Occupied(_) => bail!("insert of {} twice", key),
            Entry::Vacant(v) => {
                v.insert(FuncParamRetTypes::new_with_reserverd_storage(ty_list, index, key));
            }
        }
        Ok(())
    }

    /// Get the FuncParamType given a key
    pub fn get(&self, key: &str) -> Option<&FuncParamRetTypes> {
        self.0.get(&key.to_string())
    }
}